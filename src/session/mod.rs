mod auth;
pub mod internals;

use std::collections::HashMap;
use std::time::Duration;

use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::connection::Connection;
use crate::connection::transport::intermediate;
use crate::crypto::{mtproto, random, sha1};
use crate::errors::{Error, Result, RpcError};
use crate::raw::core::{gzip_packed, msg_container};
use crate::raw::{Deserializable, Identifiable, RemoteCall, Serializable, base, functions, types};
use internals::{MsgId, SeqNo, data_center};

const RPC_RESULT: u32 = 0xf35c6d01;
const RPC_ERROR: u32 = <types::RpcError as Identifiable>::CONSTRUCTOR_ID;
const BAD_SERVER_SALT: u32 = <types::BadServerSalt as Identifiable>::CONSTRUCTOR_ID;
const BAD_MSG_NOTIFICATION: u32 = <types::BadMsgNotification as Identifiable>::CONSTRUCTOR_ID;
const NEW_SESSION_CREATED: u32 = <types::NewSessionCreated as Identifiable>::CONSTRUCTOR_ID;
const PONG: u32 = <types::Pong as Identifiable>::CONSTRUCTOR_ID;

const HANDSHAKE_ATTEMPTS: usize = 3;
const PING_EVERY: Duration = Duration::from_secs(60);
const ACKS_THRESHOLD: usize = 10;
const UPDATES_BUFFER: usize = 1024;

type Answer = Result<Vec<u8>>;

struct Request {
    body: Vec<u8>,
    answer: oneshot::Sender<Answer>,
}

#[derive(Clone)]
pub struct Session {
    requests: mpsc::UnboundedSender<Request>,
    dc_id: i32,
    auth_key: [u8; 256],
}

impl Session {
    pub async fn connect(dc_id: i32, auth_key: Option<[u8; 256]>) -> Result<(Session, mpsc::Receiver<base::Updates>)> {
        let addr = data_center::address(dc_id).ok_or(Error::InvalidDc(dc_id))?;
        let (conn, auth_key, salt, time_offset) = match auth_key {
            Some(key) => (Connection::connect(addr).await?, key, 0, 0),
            None => {
                let mut attempt = 1;
                loop {
                    let mut conn = Connection::connect(addr).await?;
                    match auth::create(&mut conn, dc_id).await {
                        Ok(c) => break (conn, c.auth_key, c.salt, c.time_offset),
                        Err(e) if attempt == HANDSHAKE_ATTEMPTS => return Err(e),
                        Err(_) => attempt += 1,
                    }
                }
            }
        };

        let (reader, writer) = conn.into_split();
        let (packets_tx, packets) = mpsc::channel(64);
        let reader_task = tokio::spawn(read_packets(reader, packets_tx));
        let (requests_tx, requests) = mpsc::unbounded_channel();
        let (updates_tx, updates) = mpsc::channel(UPDATES_BUFFER);
        let runner = Runner {
            writer,
            auth_key,
            auth_key_id: sha1(&[&auth_key])[12..].try_into().unwrap(),
            session_id: random(),
            salt,
            msg_ids: MsgId::new(time_offset),
            seq: SeqNo::default(),
            pending: HashMap::new(),
            acks: Vec::new(),
            updates: updates_tx,
        };
        tokio::spawn(runner.run(packets, requests, reader_task));
        Ok((Session { requests: requests_tx, dc_id, auth_key }, updates))
    }

    pub fn dc_id(&self) -> i32 {
        self.dc_id
    }

    pub fn auth_key(&self) -> &[u8; 256] {
        &self.auth_key
    }

    pub async fn invoke<R: RemoteCall>(&self, request: &R) -> Result<R::Return> {
        let (answer, rx) = oneshot::channel();
        self.requests.send(Request { body: request.to_bytes(), answer }).map_err(|_| Error::Disconnected)?;
        let body = rx.await.map_err(|_| Error::Disconnected)??;
        Ok(R::Return::from_bytes(&body)?)
    }
}

async fn read_packets(mut reader: OwnedReadHalf, packets: mpsc::Sender<Result<Vec<u8>>>) {
    loop {
        let packet = intermediate::recv(&mut reader).await;
        let failed = packet.is_err();
        if packets.send(packet).await.is_err() || failed {
            break;
        }
    }
}

fn constructor_id(body: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(body.get(..4)?.try_into().unwrap()))
}

struct Pending {
    body: Vec<u8>,
    answer: oneshot::Sender<Answer>,
}

struct Runner {
    writer: OwnedWriteHalf,
    auth_key: [u8; 256],
    auth_key_id: [u8; 8],
    session_id: [u8; 8],
    salt: i64,
    msg_ids: MsgId,
    seq: SeqNo,
    pending: HashMap<i64, Pending>,
    acks: Vec<i64>,
    updates: mpsc::Sender<base::Updates>,
}

impl Runner {
    async fn run(
        mut self,
        mut packets: mpsc::Receiver<Result<Vec<u8>>>,
        mut requests: mpsc::UnboundedReceiver<Request>,
        reader_task: JoinHandle<()>,
    ) {
        let mut ping = tokio::time::interval(PING_EVERY);
        let reason = loop {
            let step = tokio::select! {
                request = requests.recv() => match request {
                    Some(r) => self.send_request(r).await,
                    None => break Error::Disconnected,
                },
                packet = packets.recv() => match packet {
                    Some(Ok(p)) => self.handle_packet(&p).await,
                    Some(Err(e)) => Err(e),
                    None => Err(Error::Disconnected),
                },
                _ = ping.tick() => self.ping().await,
            };
            if let Err(e) = step {
                break e;
            }
        };
        reader_task.abort();
        let code = match reason {
            Error::Transport(code) => Some(code),
            _ => None,
        };
        for (_, p) in self.pending.drain() {
            let _ = p.answer.send(Err(code.map_or(Error::Disconnected, Error::Transport)));
        }
    }

    async fn send(&mut self, body: &[u8], content_related: bool) -> Result<i64> {
        let msg_id = self.msg_ids.next();
        let seq_no = self.seq.next(content_related);
        let packet =
            mtproto::pack(&self.auth_key, &self.auth_key_id, &self.session_id, self.salt, msg_id, seq_no, body);
        intermediate::send(&mut self.writer, &packet).await?;
        Ok(msg_id)
    }

    async fn send_request(&mut self, r: Request) -> Result<()> {
        self.flush_acks().await?;
        let msg_id = self.send(&r.body, true).await?;
        self.pending.insert(msg_id, Pending { body: r.body, answer: r.answer });
        Ok(())
    }

    async fn resend(&mut self, bad_msg_id: i64) -> Result<()> {
        if let Some(p) = self.pending.remove(&bad_msg_id) {
            let msg_id = self.send(&p.body, true).await?;
            self.pending.insert(msg_id, p);
        }
        Ok(())
    }

    async fn flush_acks(&mut self) -> Result<()> {
        if self.acks.is_empty() {
            return Ok(());
        }
        let ack = types::MsgsAck { msg_ids: std::mem::take(&mut self.acks) };
        self.send(&ack.to_bytes(), false).await.map(drop)
    }

    async fn ping(&mut self) -> Result<()> {
        self.flush_acks().await?;
        let ping = functions::PingDelayDisconnect { ping_id: i64::from_le_bytes(random()), disconnect_delay: 75 };
        self.send(&ping.to_bytes(), true).await.map(drop)
    }

    fn answer(&mut self, msg_id: i64, answer: Answer) {
        if let Some(p) = self.pending.remove(&msg_id) {
            let _ = p.answer.send(answer);
        }
    }

    async fn handle_packet(&mut self, packet: &[u8]) -> Result<()> {
        // Drop anything that fails decryption or the MTProto checks: it isn't from the server.
        let Ok(msg) = mtproto::unpack(packet, &self.auth_key, &self.auth_key_id, &self.session_id, true) else {
            return Ok(());
        };
        if msg.msg_id % 2 == 0 {
            return Ok(());
        }
        if constructor_id(&msg.body) == Some(msg_container::ID) {
            for m in msg_container::parse(&msg.body).unwrap_or_default() {
                self.handle_message(m.msg_id, m.seq_no, m.body).await?;
            }
        } else {
            self.handle_message(msg.msg_id, msg.seq_no, &msg.body).await?;
        }
        if self.acks.len() >= ACKS_THRESHOLD {
            self.flush_acks().await?;
        }
        Ok(())
    }

    async fn handle_message(&mut self, msg_id: i64, seq_no: i32, body: &[u8]) -> Result<()> {
        if seq_no % 2 == 1 {
            self.acks.push(msg_id);
        }
        let unpacked;
        let body = if constructor_id(body) == Some(gzip_packed::ID) {
            let Ok(b) = gzip_packed::unpack(body) else { return Ok(()) };
            unpacked = b;
            &unpacked[..]
        } else {
            body
        };

        match constructor_id(body) {
            Some(RPC_RESULT) => self.handle_rpc_result(body),
            Some(BAD_SERVER_SALT | BAD_MSG_NOTIFICATION) => self.handle_bad_msg(msg_id, body).await?,
            Some(NEW_SESSION_CREATED) => {
                if let Ok(base::NewSession::NewSessionCreated(s)) = base::NewSession::from_bytes(body) {
                    self.salt = s.server_salt;
                }
            }
            Some(PONG) => {
                if let Ok(base::Pong::Pong(p)) = base::Pong::from_bytes(body) {
                    self.answer(p.msg_id, Ok(body.to_vec()));
                }
            }
            _ => {
                if let Ok(u) = base::Updates::from_bytes(body) {
                    let _ = self.updates.try_send(u);
                }
            }
        }
        Ok(())
    }

    fn handle_rpc_result(&mut self, body: &[u8]) {
        let Some(req_msg_id) = body.get(4..12).map(|b| i64::from_le_bytes(b.try_into().unwrap())) else {
            return;
        };
        let mut result = body[12..].to_vec();
        if constructor_id(&result) == Some(gzip_packed::ID) {
            match gzip_packed::unpack(&result) {
                Ok(r) => result = r,
                Err(e) => return self.answer(req_msg_id, Err(e.into())),
            }
        }
        let answer = if constructor_id(&result) == Some(RPC_ERROR) {
            match base::RpcError::from_bytes(&result) {
                Ok(base::RpcError::RpcError(e)) => {
                    Err(Error::Rpc(RpcError::new(e.error_code, &String::from_utf8_lossy(&e.error_message))))
                }
                Err(e) => Err(e.into()),
            }
        } else {
            Ok(result)
        };
        self.answer(req_msg_id, answer);
    }

    async fn handle_bad_msg(&mut self, server_msg_id: i64, body: &[u8]) -> Result<()> {
        match base::BadMsgNotification::from_bytes(body) {
            Ok(base::BadMsgNotification::BadServerSalt(b)) => {
                self.salt = b.new_server_salt;
                self.resend(b.bad_msg_id).await?;
            }
            Ok(base::BadMsgNotification::BadMsgNotification(b)) => match b.error_code {
                // Our msg_id is too low or too high: the local clock is off. The server's
                // own msg_id carries its time, so take the offset from it and resend.
                16 | 17 => {
                    self.msg_ids.time_offset = (server_msg_id >> 32) - internals::now();
                    self.resend(b.bad_msg_id).await?;
                }
                code => self.answer(b.bad_msg_id, Err(Error::BadMessage(code))),
            },
            Err(_) => {}
        }
        Ok(())
    }
}
