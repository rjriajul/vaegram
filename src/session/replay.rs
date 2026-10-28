use std::borrow::Cow;
use std::collections::{HashMap, VecDeque};
use std::io;
use std::net::SocketAddr;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;

use super::internals::now;
use super::{MSG_RESEND_REQ, MSGS_ACK, NEW_SESSION_CREATED, RPC_RESULT, constructor_id};
use crate::connection::Transport;
use crate::crypto::{aes, mtproto, random, sha1, sha256};
use crate::errors::Result;
use crate::raw::core::{gzip_packed, msg_container};
use crate::raw::{Deserializable, Identifiable, Serializable, base, functions, types};
use crate::storage::recording::Record;
use crate::storage::{SessionData, session_string};

const PING: u32 = <functions::Ping as Identifiable>::CONSTRUCTOR_ID;
const PING_DELAY_DISCONNECT: u32 = <functions::PingDelayDisconnect as Identifiable>::CONSTRUCTOR_ID;
const RPC_DROP_ANSWER: u32 = <functions::RpcDropAnswer as Identifiable>::CONSTRUCTOR_ID;
const MSGS_STATE_INFO: u32 = <types::MsgsStateInfo as Identifiable>::CONSTRUCTOR_ID;
// Keepalive traffic varies from run to run, so it's answered live or ignored, never from the recording.
const SERVICE: [u32; 6] = [PING, PING_DELAY_DISCONNECT, MSGS_ACK, MSG_RESEND_REQ, RPC_DROP_ANSWER, MSGS_STATE_INFO];

/// Answers each request with the next recorded result for its method. Unasked messages wait until as many
/// answers have gone out as when they were recorded, so a client that strays stops getting updates.
pub struct Replay {
    addr: SocketAddr,
    auth_key: [u8; 256],
    server: JoinHandle<()>,
}

impl Replay {
    pub async fn start(records: &[Record]) -> io::Result<Replay> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let auth_key = random();
        let mut script = Script::new(records);
        let server = tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let _ = script.serve(stream, &auth_key).await;
            }
        });
        Ok(Replay { addr, auth_key, server })
    }

    /// The replay has no user: use `connect()`, not `start()`.
    pub fn session_string(&self) -> String {
        let (addr, auth_key) = (Some(self.addr), self.auth_key);
        session_string::export(&SessionData {
            dc_id: 2,
            test_mode: false,
            addr,
            api_id: 0,
            auth_key,
            user_id: 0,
            is_bot: false,
        })
    }
}

impl Drop for Replay {
    fn drop(&mut self) {
        self.server.abort();
    }
}

struct Script {
    answers: HashMap<u32, VecDeque<Vec<u8>>>,
    pushes: VecDeque<(usize, Vec<u8>)>,
    answered: usize,
    last_msg_id: i64,
}

impl Script {
    fn new(records: &[Record]) -> Script {
        let methods: HashMap<i64, u32> = records
            .iter()
            .filter(|r| r.outgoing)
            .filter_map(|r| Some((r.msg_id, constructor_id(&r.body)?)))
            .filter(|(_, id)| !SERVICE.contains(id))
            .collect();
        let mut results = HashMap::new();
        let mut pushes = VecDeque::new();
        for r in records.iter().filter(|r| !r.outgoing) {
            let body = unpacked(&r.body);
            match constructor_id(&body) {
                Some(RPC_RESULT) => {
                    let Some(req_msg_id) = body.get(4..12).map(|b| i64::from_le_bytes(b.try_into().unwrap())) else {
                        continue;
                    };
                    if methods.contains_key(&req_msg_id) {
                        results.entry(req_msg_id).or_insert_with(|| body[12..].to_vec());
                    }
                }
                Some(NEW_SESSION_CREATED) => pushes.push_back((results.len(), r.body.clone())),
                _ if base::Updates::from_bytes(&body).is_ok() => pushes.push_back((results.len(), r.body.clone())),
                _ => {}
            }
        }
        let mut answers: HashMap<u32, VecDeque<Vec<u8>>> = HashMap::new();
        for r in records.iter().filter(|r| r.outgoing) {
            if let (Some(&method), Some(result)) = (methods.get(&r.msg_id), results.remove(&r.msg_id)) {
                answers.entry(method).or_default().push_back(result);
            }
        }
        Script { answers, pushes, answered: 0, last_msg_id: 0 }
    }

    async fn serve(&mut self, mut stream: TcpStream, auth_key: &[u8; 256]) -> Result<()> {
        let mut tag = [0; 4];
        stream.read_exact(&mut tag[..1]).await?;
        let transport = if tag[0] == 0xef {
            Transport::Abridged
        } else {
            stream.read_exact(&mut tag[1..]).await?;
            match tag {
                [0xee, 0xee, 0xee, 0xee] => Transport::Intermediate,
                [0xdd, 0xdd, 0xdd, 0xdd] => Transport::PaddedIntermediate,
                _ => return Ok(()),
            }
        };
        let auth_key_id: [u8; 8] = sha1(&[&auth_key[..]])[12..].try_into().unwrap();
        let (mut out, mut framed, mut content) = (Vec::new(), Vec::new(), 0);
        loop {
            let mut packet = transport.read(&mut stream).await?;
            let Some((session_id, msg_id, body)) = open(auth_key, &auth_key_id, &mut packet) else { continue };
            if constructor_id(body) == Some(msg_container::ID) {
                for m in msg_container::parse(body).unwrap_or_default() {
                    self.answer(m.msg_id, m.body, &mut out);
                }
            } else {
                self.answer(msg_id, body, &mut out);
            }
            while self.pushes.front().is_some_and(|(after, _)| *after <= self.answered) {
                out.extend(self.pushes.pop_front().map(|(_, body)| body));
            }
            for body in out.drain(..) {
                content += 1;
                let packet = seal(auth_key, &auth_key_id, &session_id, self.msg_id(), content * 2 - 1, &body);
                framed.clear();
                transport.encode(&packet, &mut framed);
                stream.write_all(&framed).await?;
            }
        }
    }

    fn answer(&mut self, msg_id: i64, body: &[u8], out: &mut Vec<Vec<u8>>) {
        let Some(id) = constructor_id(body) else { return };
        let result = match id {
            PING | PING_DELAY_DISCONNECT => {
                let ping_id = body.get(4..12).map_or(0, |b| i64::from_le_bytes(b.try_into().unwrap()));
                return out.push(base::Pong::Pong(Box::new(types::Pong { msg_id, ping_id })).to_bytes());
            }
            _ if SERVICE.contains(&id) => return,
            _ => match self.answers.get_mut(&id).and_then(VecDeque::pop_front) {
                Some(result) => {
                    self.answered += 1;
                    result
                }
                None => {
                    // The caller sees which request ran past the recording as the error's value.
                    let error_message = format!("REPLAY_EXHAUSTED_{id}").into_bytes();
                    base::RpcError::RpcError(Box::new(types::RpcError { error_code: 400, error_message })).to_bytes()
                }
            },
        };
        let mut b = Vec::with_capacity(12 + result.len());
        b.extend_from_slice(&RPC_RESULT.to_le_bytes());
        b.extend_from_slice(&msg_id.to_le_bytes());
        b.extend_from_slice(&result);
        out.push(b);
    }

    fn msg_id(&mut self) -> i64 {
        self.last_msg_id = (self.last_msg_id + 4).max((now() << 32) | 1);
        self.last_msg_id
    }
}

fn unpacked(body: &[u8]) -> Cow<'_, [u8]> {
    match constructor_id(body) {
        Some(gzip_packed::ID) => gzip_packed::unpack(body).map_or(Cow::Borrowed(body), Cow::Owned),
        _ => Cow::Borrowed(body),
    }
}

/// MTProto 2.0 from the server's side: the msg_key of what it sends hashes auth_key[96..128].
fn seal(
    auth_key: &[u8; 256],
    auth_key_id: &[u8; 8],
    session_id: &[u8; 8],
    msg_id: i64,
    seq_no: i32,
    body: &[u8],
) -> Vec<u8> {
    let padding = 12 + (16 - (32 + body.len() + 12) % 16) % 16;
    let mut packet = Vec::with_capacity(56 + body.len() + padding);
    packet.extend_from_slice(auth_key_id);
    packet.extend_from_slice(&[0; 24]);
    packet.extend_from_slice(session_id);
    packet.extend_from_slice(&msg_id.to_le_bytes());
    packet.extend_from_slice(&seq_no.to_le_bytes());
    packet.extend_from_slice(&(body.len() as u32).to_le_bytes());
    packet.extend_from_slice(body);
    packet.resize(packet.len() + padding, 0);
    let msg_key: [u8; 16] = sha256(&[&auth_key[96..128], &packet[24..]])[8..24].try_into().unwrap();
    packet[8..24].copy_from_slice(&msg_key);
    let (key, iv) = mtproto::kdf(auth_key, &msg_key, false);
    aes::ige256_encrypt(&mut packet[24..], &key, &iv).unwrap();
    packet
}

fn open<'a>(auth_key: &[u8; 256], auth_key_id: &[u8; 8], packet: &'a mut [u8]) -> Option<([u8; 8], i64, &'a [u8])> {
    if packet.len() < 56 || (packet.len() - 24) % 16 != 0 || packet[..8] != auth_key_id[..] {
        return None;
    }
    let msg_key: [u8; 16] = packet[8..24].try_into().unwrap();
    let (key, iv) = mtproto::kdf(auth_key, &msg_key, true);
    aes::ige256_decrypt(&mut packet[24..], &key, &iv).ok()?;
    if sha256(&[&auth_key[88..120], &packet[24..]])[8..24] != msg_key {
        return None;
    }
    let packet: &'a [u8] = packet;
    let len = u32::from_le_bytes(packet[52..56].try_into().unwrap()) as usize;
    let msg_id = i64::from_le_bytes(packet[40..48].try_into().unwrap());
    Some((packet[32..40].try_into().unwrap(), msg_id, packet[56..].get(..len)?))
}
