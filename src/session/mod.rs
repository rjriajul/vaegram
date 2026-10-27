mod auth;
pub mod internals;
mod outbox;
mod service;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::connection::{Connection, Options, Reader, Writer};
use crate::crypto::{mtproto, random, sha1};
use crate::errors::{Error, Result, RpcError};
use crate::raw::core::{gzip_packed, msg_container};
use crate::raw::{Deserializable, Identifiable, RemoteCall, Serializable, base, functions, types};
use internals::data_center::Dc;
use internals::{MsgId, SeqNo};
use outbox::MAX_CONTAINER_MSGS;

const RPC_RESULT: u32 = 0xf35c6d01;
const RPC_ERROR: u32 = <types::RpcError as Identifiable>::CONSTRUCTOR_ID;
const BAD_SERVER_SALT: u32 = <types::BadServerSalt as Identifiable>::CONSTRUCTOR_ID;
const BAD_MSG_NOTIFICATION: u32 = <types::BadMsgNotification as Identifiable>::CONSTRUCTOR_ID;
const NEW_SESSION_CREATED: u32 = <types::NewSessionCreated as Identifiable>::CONSTRUCTOR_ID;
const PONG: u32 = <types::Pong as Identifiable>::CONSTRUCTOR_ID;
const MSGS_STATE_REQ: u32 = <types::MsgsStateReq as Identifiable>::CONSTRUCTOR_ID;
const MSGS_ALL_INFO: u32 = <types::MsgsAllInfo as Identifiable>::CONSTRUCTOR_ID;
const MSG_DETAILED_INFO: u32 = <types::MsgDetailedInfo as Identifiable>::CONSTRUCTOR_ID;
const MSG_NEW_DETAILED_INFO: u32 = <types::MsgNewDetailedInfo as Identifiable>::CONSTRUCTOR_ID;
const MSG_RESEND_REQ: u32 = <types::MsgResendReq as Identifiable>::CONSTRUCTOR_ID;
const MSGS_ACK: u32 = <types::MsgsAck as Identifiable>::CONSTRUCTOR_ID;

const HANDSHAKE_ATTEMPTS: usize = 3;
const PING_EVERY: Duration = Duration::from_secs(60);
const ACKS_THRESHOLD: usize = 10;
const UPDATES_BUFFER: usize = 1024;
const RECONNECT_DELAY: Duration = Duration::from_millis(500);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(30);
const TICK: Duration = Duration::from_secs(1);
const ACK_EVERY: Duration = Duration::from_secs(15);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

type Answer = Result<Vec<u8>>;

struct Request {
    body: Vec<u8>,
    answer: oneshot::Sender<Answer>,
}

pub struct Incoming {
    pub updates: base::Updates,
    pub from_request: bool,
}

/// The session's updates. Ones the runner had to drop on a full channel come back as one `UpdatesTooLong`.
pub struct UpdatesRx {
    rx: mpsc::Receiver<Incoming>,
    dropped: Arc<AtomicU32>,
}

impl UpdatesRx {
    pub async fn recv(&mut self) -> Option<Incoming> {
        if self.dropped.swap(0, Ordering::AcqRel) > 0 {
            return Some(Incoming { updates: base::Updates::UpdatesTooLong, from_request: false });
        }
        self.rx.recv().await
    }

    #[cfg(test)]
    pub(crate) fn offline() -> UpdatesRx {
        UpdatesRx { rx: mpsc::channel(1).1, dropped: Arc::default() }
    }
}

#[derive(Clone)]
pub struct Session {
    requests: mpsc::UnboundedSender<Request>,
    dc_id: i32,
    auth_key: [u8; 256],
}

impl Session {
    pub async fn connect(dc: Dc, auth_key: Option<[u8; 256]>, options: &Options) -> Result<(Session, UpdatesRx)> {
        let (conn, auth_key, salt, time_offset) = match auth_key {
            Some(key) => (Connection::connect(&dc, options).await?, key, 0, 0),
            None => {
                let mut attempt = 1;
                loop {
                    let mut conn = Connection::connect(&dc, options).await?;
                    match auth::create(&mut conn, dc).await {
                        Ok(c) => break (conn, c.auth_key, c.salt, c.time_offset),
                        Err(e) if attempt == HANDSHAKE_ATTEMPTS => return Err(e),
                        Err(_) => attempt += 1,
                    }
                }
            }
        };

        let (requests_tx, requests) = mpsc::unbounded_channel();
        let (runner, updates) = Runner::new(dc, options.clone(), conn, auth_key, salt, time_offset);
        tokio::spawn(runner.run(requests));
        Ok((Session { requests: requests_tx, dc_id: dc.id, auth_key }, updates))
    }

    #[cfg(test)]
    pub(crate) fn offline() -> Session {
        Session { requests: mpsc::unbounded_channel().0, dc_id: 2, auth_key: [0; 256] }
    }

    #[cfg(test)]
    pub(crate) fn scripted(replies: Vec<Answer>) -> (Session, mpsc::UnboundedReceiver<Vec<u8>>) {
        let (requests, mut rx) = mpsc::unbounded_channel::<Request>();
        let (bodies, sent) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut replies = replies.into_iter();
            while let Some(request) = rx.recv().await {
                let _ = bodies.send(request.body);
                if let Some(reply) = replies.next() {
                    let _ = request.answer.send(reply);
                }
            }
        });
        (Session { requests, dc_id: 2, auth_key: [0; 256] }, sent)
    }

    pub fn dc_id(&self) -> i32 {
        self.dc_id
    }

    pub fn auth_key(&self) -> &[u8; 256] {
        &self.auth_key
    }

    /// Dropping the future abandons the request.
    pub async fn invoke<R: RemoteCall>(&self, request: &R) -> Result<R::Return> {
        let (answer, rx) = oneshot::channel();
        self.requests.send(Request { body: request.to_bytes(), answer }).map_err(|_| Error::Disconnected)?;
        let answer = tokio::time::timeout(REQUEST_TIMEOUT, rx).await.map_err(|_| Error::Timeout)?;
        let body = answer.map_err(|_| Error::Disconnected)??;
        Ok(R::Return::from_bytes(&body)?)
    }
}

fn start_reading(conn: Connection) -> (Writer, mpsc::Receiver<Result<Vec<u8>>>, JoinHandle<()>) {
    let (reader, writer) = conn.into_split();
    let (packets_tx, packets) = mpsc::channel(64);
    (writer, packets, tokio::spawn(read_packets(reader, packets_tx)))
}

async fn read_packets(mut reader: Reader, packets: mpsc::Sender<Result<Vec<u8>>>) {
    loop {
        let packet = reader.recv().await;
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
    container: i64,
    // The server confirmed receipt; after a reconnect it re-sends the answer by itself.
    acked: bool,
}

struct Runner {
    dc: Dc,
    options: Options,
    writer: Writer,
    packets: mpsc::Receiver<Result<Vec<u8>>>,
    reader: JoinHandle<()>,
    auth_key: [u8; 256],
    auth_key_id: [u8; 8],
    session_id: [u8; 8],
    salt: i64,
    msg_ids: MsgId,
    seq: SeqNo,
    pending: HashMap<i64, Pending>,
    queue: VecDeque<Pending>,
    service: VecDeque<(Vec<u8>, bool)>,
    acks: Vec<i64>,
    buf: Vec<u8>,
    updates: mpsc::Sender<Incoming>,
    dropped: Arc<AtomicU32>,
}

impl Runner {
    fn new(
        dc: Dc,
        options: Options,
        conn: Connection,
        auth_key: [u8; 256],
        salt: i64,
        time_offset: i64,
    ) -> (Runner, UpdatesRx) {
        let (writer, packets, reader) = start_reading(conn);
        let (updates_tx, rx) = mpsc::channel(UPDATES_BUFFER);
        let dropped = Arc::<AtomicU32>::default();
        let runner = Runner {
            dc,
            options,
            writer,
            packets,
            reader,
            auth_key,
            auth_key_id: sha1(&[&auth_key])[12..].try_into().unwrap(),
            session_id: random(),
            salt,
            msg_ids: MsgId::new(time_offset),
            seq: SeqNo::default(),
            pending: HashMap::new(),
            queue: VecDeque::new(),
            service: VecDeque::new(),
            acks: Vec::new(),
            buf: Vec::new(),
            updates: updates_tx,
            dropped: dropped.clone(),
        };
        (runner, UpdatesRx { rx, dropped })
    }

    async fn run(mut self, mut requests: mpsc::UnboundedReceiver<Request>) {
        let mut ping = tokio::time::interval(PING_EVERY);
        let mut tick = tokio::time::interval(TICK);
        let mut ack = tokio::time::interval(ACK_EVERY);
        let reason = loop {
            let step = tokio::select! {
                request = requests.recv() => match request {
                    Some(r) => {
                        self.enqueue(r);
                        while self.queue.len() < MAX_CONTAINER_MSGS {
                            let Ok(r) = requests.try_recv() else { break };
                            self.enqueue(r);
                        }
                        self.flush().await
                    }
                    None => break Error::Disconnected,
                },
                packet = self.packets.recv() => match packet {
                    Some(Ok(p)) => self.handle_packet(&p).await,
                    Some(Err(e)) => Err(e),
                    None => Err(Error::Disconnected),
                },
                _ = ping.tick() => self.ping().await,
                _ = tick.tick() => {
                    self.drop_abandoned();
                    if self.service.is_empty() { Ok(()) } else { self.flush().await }
                }
                _ = ack.tick() => self.flush().await,
            };
            match step {
                Ok(()) => {}
                Err(e @ Error::Transport(404)) => break e,
                Err(_) => {
                    self.reconnect().await;
                    ping.reset();
                }
            }
        };
        self.reader.abort();
        let code = match reason {
            Error::Transport(code) => Some(code),
            _ => None,
        };
        for p in self.pending.drain().map(|(_, p)| p).chain(self.queue.drain(..)) {
            let _ = p.answer.send(Err(code.map_or(Error::Disconnected, Error::Transport)));
        }
    }

    async fn reconnect(&mut self) {
        let mut delay = RECONNECT_DELAY;
        while !self.updates.is_closed() {
            self.reader.abort();
            tokio::time::sleep(delay).await;
            delay = (delay * 2).min(MAX_RECONNECT_DELAY);
            self.drop_abandoned();
            let Ok(conn) = Connection::connect(&self.dc, &self.options).await else { continue };
            (self.writer, self.packets, self.reader) = start_reading(conn);
            if self.resend_pending().await.is_ok() {
                self.request_difference();
                return;
            }
        }
    }

    fn request_difference(&self) {
        self.forward(Incoming { updates: base::Updates::UpdatesTooLong, from_request: false });
    }

    fn forward(&self, incoming: Incoming) {
        if let Err(mpsc::error::TrySendError::Full(_)) = self.updates.try_send(incoming) {
            self.dropped.fetch_add(1, Ordering::Release);
        }
    }

    async fn resend_pending(&mut self) -> Result<()> {
        // Re-sending an acked request could run it twice.
        let unacked: Vec<i64> = self.pending.iter().filter(|(_, p)| !p.acked).map(|(&id, _)| id).collect();
        for id in unacked {
            self.queue.extend(self.pending.remove(&id));
        }
        self.ping().await
    }

    fn enqueue(&mut self, r: Request) {
        if !r.answer.is_closed() {
            self.queue.push_back(Pending { body: r.body, answer: r.answer, container: 0, acked: false });
        }
    }

    fn drop_abandoned(&mut self) {
        let gone: Vec<i64> = self.pending.iter().filter(|(_, p)| p.answer.is_closed()).map(|(&id, _)| id).collect();
        for req_msg_id in gone {
            self.pending.remove(&req_msg_id);
            self.service.push_back((functions::RpcDropAnswer { req_msg_id }.to_bytes(), true));
        }
    }

    async fn ping(&mut self) -> Result<()> {
        let ping = functions::PingDelayDisconnect { ping_id: i64::from_le_bytes(random()), disconnect_delay: 75 };
        self.service.push_back((ping.to_bytes(), true));
        self.flush().await
    }

    fn answer(&mut self, msg_id: i64, answer: Answer) {
        if let Some(p) = self.pending.remove(&msg_id) {
            let _ = p.answer.send(answer);
        }
    }

    fn take_sent(&mut self, msg_id: i64) -> Vec<Pending> {
        if let Some(p) = self.pending.remove(&msg_id) {
            return vec![p];
        }
        let ids: Vec<i64> = self.pending.iter().filter(|(_, p)| p.container == msg_id).map(|(&id, _)| id).collect();
        ids.iter().filter_map(|id| self.pending.remove(id)).collect()
    }

    fn resend(&mut self, msg_id: i64) {
        let sent = self.take_sent(msg_id);
        self.queue.extend(sent);
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
                self.handle_message(m.msg_id, m.seq_no, m.body);
            }
        } else {
            self.handle_message(msg.msg_id, msg.seq_no, &msg.body);
        }
        if self.acks.len() >= ACKS_THRESHOLD || !self.queue.is_empty() || !self.service.is_empty() {
            self.flush().await?;
        }
        Ok(())
    }

    fn handle_message(&mut self, msg_id: i64, seq_no: i32, body: &[u8]) {
        if seq_no % 2 == 1 {
            self.acks.push(msg_id);
        }
        let unpacked;
        let body = if constructor_id(body) == Some(gzip_packed::ID) {
            let Ok(b) = gzip_packed::unpack(body) else { return };
            unpacked = b;
            &unpacked[..]
        } else {
            body
        };

        match constructor_id(body) {
            Some(RPC_RESULT) => self.handle_rpc_result(body),
            Some(BAD_SERVER_SALT | BAD_MSG_NOTIFICATION) => self.handle_bad_msg(msg_id, body),
            Some(NEW_SESSION_CREATED) => self.handle_new_session(body),
            Some(MSGS_ACK) => self.handle_msgs_ack(body),
            Some(PONG) => {
                if let Ok(base::Pong::Pong(p)) = base::Pong::from_bytes(body) {
                    self.answer(p.msg_id, Ok(body.to_vec()));
                }
            }
            Some(MSGS_STATE_REQ) => self.handle_state_req(msg_id, body),
            Some(MSG_RESEND_REQ) => self.handle_resend_req(msg_id, body),
            Some(MSGS_ALL_INFO) => self.handle_all_info(body),
            Some(MSG_DETAILED_INFO | MSG_NEW_DETAILED_INFO) => self.handle_detailed_info(body),
            _ => {
                if let Ok(updates) = base::Updates::from_bytes(body) {
                    self.forward(Incoming { updates, from_request: false });
                }
            }
        }
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
            if let Ok(updates) = base::Updates::from_bytes(&result) {
                self.forward(Incoming { updates, from_request: true });
            }
            Ok(result)
        };
        self.answer(req_msg_id, answer);
    }
}
