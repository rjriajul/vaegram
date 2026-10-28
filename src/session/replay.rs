use std::borrow::Cow;
use std::collections::{HashMap, HashSet, VecDeque};
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::{JoinHandle, JoinSet};

use super::internals::now;
use super::{MSG_RESEND_REQ, MSGS_ACK, NEW_SESSION_CREATED, RPC_RESULT, constructor_id};
use crate::connection::{RecvBuf, Transport};
use crate::crypto::{aes, mtproto, random, sha1, sha256};
use crate::errors::Result;
use crate::raw::core::{gzip_packed, msg_container};
use crate::raw::{Deserializable, Identifiable, Serializable, base, functions, types};
use crate::storage::recording::Record;
use crate::storage::{SessionData, session_string};

const PING: u32 = <functions::Ping as Identifiable>::CONSTRUCTOR_ID;
const PING_DELAY_DISCONNECT: u32 = <functions::PingDelayDisconnect as Identifiable>::CONSTRUCTOR_ID;
const RPC_DROP_ANSWER: u32 = <functions::RpcDropAnswer as Identifiable>::CONSTRUCTOR_ID;
const INVOKE_WITH_LAYER: u32 = <functions::InvokeWithLayer<functions::Ping> as Identifiable>::CONSTRUCTOR_ID;
const MSGS_STATE_INFO: u32 = <types::MsgsStateInfo as Identifiable>::CONSTRUCTOR_ID;
// Keepalive traffic varies from run to run, so it's answered live or ignored, never from the recording.
const SERVICE: [u32; 6] = [PING, PING_DELAY_DISCONNECT, MSGS_ACK, MSG_RESEND_REQ, RPC_DROP_ANSWER, MSGS_STATE_INFO];

/// Answers each request with the next recorded result for its method. Unasked messages wait until as many
/// answers have gone out as when they were recorded, so a client that strays stops getting updates; they go to
/// the client's main connection only. A request the recording has no more answers for gets the error
/// `REPLAY_EXHAUSTED_<constructor id>`. The replay can't run a key exchange, so a client with `.pfs()` or one
/// that opens a connection to another data center can't use it.
pub struct Replay {
    addr: SocketAddr,
    auth_key: [u8; 256],
    server: JoinHandle<()>,
}

impl Replay {
    /// Listens on a free localhost port; connections are served side by side, like a data center's.
    pub async fn start(records: &[Record]) -> io::Result<Replay> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        let auth_key = random();
        let script = Arc::new(Mutex::new(Script::new(records)));
        let server = tokio::spawn(async move {
            // Dropped with this task, which ends every connection along with it.
            let mut connections = JoinSet::new();
            while let Ok((stream, _)) = listener.accept().await {
                while connections.try_join_next().is_some() {}
                let script = script.clone();
                connections.spawn(async move { serve(stream, &script, &auth_key).await });
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
    // Sessions that sent initConnection: the client's main connection, the one that takes updates.
    main: HashSet<[u8; 8]>,
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
        Script { answers, pushes, answered: 0, last_msg_id: 0, main: HashSet::new() }
    }

    /// The messages that answer `body`, followed by the pushes now due when `session_id` takes them.
    fn reply(&mut self, session_id: [u8; 8], msg_id: i64, body: &[u8]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();
        if constructor_id(body) == Some(msg_container::ID) {
            for m in msg_container::parse(body).unwrap_or_default() {
                self.answer(session_id, m.msg_id, m.body, &mut out);
            }
        } else {
            self.answer(session_id, msg_id, body, &mut out);
        }
        if self.main.contains(&session_id) {
            while self.pushes.front().is_some_and(|(after, _)| *after <= self.answered) {
                out.extend(self.pushes.pop_front().map(|(_, body)| body));
            }
        }
        out
    }

    fn answer(&mut self, session_id: [u8; 8], msg_id: i64, body: &[u8], out: &mut Vec<Vec<u8>>) {
        let Some(id) = constructor_id(body) else { return };
        if id == INVOKE_WITH_LAYER {
            self.main.insert(session_id);
        }
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

async fn serve(mut stream: TcpStream, script: &Mutex<Script>, auth_key: &[u8; 256]) -> Result<()> {
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
    let (mut framed, mut content) = (Vec::new(), 0);
    let mut buf = RecvBuf::default();
    loop {
        let Some(mut packet) = transport.decode(&mut buf)? else {
            if stream.read_buf(buf.make_room()).await? == 0 {
                return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
            }
            continue;
        };
        let Some((session_id, msg_id, body)) = open(auth_key, &auth_key_id, &mut packet) else { continue };
        let out: Vec<(i64, Vec<u8>)> = {
            let mut script = script.lock().unwrap();
            let bodies = script.reply(session_id, msg_id, body);
            bodies.into_iter().map(|body| (script.msg_id(), body)).collect()
        };
        for (msg_id, body) in out {
            content += 1;
            let packet = seal(auth_key, &auth_key_id, &session_id, msg_id, content * 2 - 1, &body);
            framed.clear();
            framed.resize(Transport::ROOM, 0);
            framed.extend_from_slice(&packet);
            let start = transport.frame(&mut framed);
            stream.write_all(&framed[start..]).await?;
        }
    }
}

fn unpacked(body: &[u8]) -> Cow<'_, [u8]> {
    match constructor_id(body) {
        Some(gzip_packed::ID) => gzip_packed::unpack(body).map_or(Cow::Borrowed(body), Cow::Owned),
        _ => Cow::Borrowed(body),
    }
}

/// MTProto 2.0 from the server's side: the msg_key of what it sends hashes auth_key[96..128].
pub(super) fn seal(
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

pub(super) fn open<'a>(
    auth_key: &[u8; 256],
    auth_key_id: &[u8; 8],
    packet: &'a mut [u8],
) -> Option<([u8; 8], i64, &'a [u8])> {
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::Replay;
    use crate::connection::Options;
    use crate::raw::functions;
    use crate::session::internals::data_center::Dc;
    use crate::session::{Known, Salt, Session};

    #[tokio::test]
    async fn serves_connections_side_by_side() {
        let replay = Replay::start(&[]).await.unwrap();
        let (dc, options) = (Dc { id: 2, addr: replay.addr, test: false }, Options::default());
        let key = Known { key: replay.auth_key, salt: Salt::default(), made: None };
        let open = |n| Session::connect_transfer(dc, Some(key), None, &options, Arc::default(), None, n);
        let (first, second) = (open(1).await.unwrap(), open(2).await.unwrap());
        for session in [&first, &second] {
            let pong = tokio::time::timeout(Duration::from_secs(10), session.invoke(&functions::Ping { ping_id: 1 }));
            pong.await.expect("a connection waited for another one to close").unwrap();
        }
    }
}
