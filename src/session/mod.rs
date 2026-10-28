mod auth;
pub(crate) mod cdn;
mod events;
pub mod internals;
mod outbox;
mod service;
#[cfg(test)]
mod tests;
pub(crate) mod transfer;

pub(crate) use events::Affected;
pub use events::Events;

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::connection::{Connection, Options, Proxy, Reader, Writer};
use crate::crypto::rsa::PublicKey;
use crate::crypto::{mtproto, random, sha1};
use crate::enums::ClientEvent;
use crate::errors::{Error, Result, RpcError};
use crate::raw::core::{gzip_packed, msg_container};
use crate::raw::{Deserializable, Identifiable, LAYER, RemoteCall, Serializable, base, functions, types};
use internals::data_center::Dc;
use internals::{MsgId, SeqNo, ServerMsgIds};
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

type Answer = Result<Vec<u8>>;

struct Request {
    name: &'static str,
    body: Vec<u8>,
    answer: oneshot::Sender<Answer>,
}

/// One push from the server, at the session level, where requests and answers are raw TL too.
pub struct Incoming {
    pub(crate) updates: base::Updates,
    pub(crate) from_request: bool,
}

impl Incoming {
    pub fn into_updates(self) -> base::Updates {
        self.updates
    }

    /// Sent back as the answer to one of this session's own requests, not pushed.
    pub fn from_request(&self) -> bool {
        self.from_request
    }
}

/// How one request ended, as reported to an [`Observer`].
#[derive(Debug)]
#[non_exhaustive]
pub struct Call<'a> {
    /// Wrappers such as `invokeWithLayer` give their own name; `request` holds the whole call.
    pub name: &'static str,
    /// The msg_id it was last sent under; 0 if it never went out.
    pub msg_id: i64,
    /// The whole call as sent, credentials included (bot tokens, SRP proofs, exported authorizations): don't log
    /// it as is. A recording keeps the redacted form.
    pub request: &'a [u8],
    pub duration: Duration,
    /// The answer's TL bytes, or the error the caller got. A caller that stopped waiting got `Error::Timeout`.
    pub result: Result<&'a [u8], &'a Error>,
}

/// The per-request hook: it runs on the connection's task for every request that ends, so it should return
/// quickly. Connection-level events (reconnects, dropped packets) go to `Client::on_event`.
#[derive(Clone)]
pub struct Observer(pub Arc<dyn Fn(&Call<'_>) + Send + Sync>);

impl std::fmt::Debug for Observer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Observer")
    }
}
pub struct UpdatesRx {
    rx: mpsc::Receiver<Incoming>,
    pub(crate) events: Arc<Events>,
}

impl UpdatesRx {
    pub async fn recv(&mut self) -> Option<Incoming> {
        let count = self.events.take_dropped();
        if count > 0 {
            self.events.emit(ClientEvent::UpdatesDropped { count });
            return Some(Incoming { updates: base::Updates::UpdatesTooLong, from_request: false });
        }
        if let Some(affected) = self.events.take_affected() {
            return Some(Incoming { updates: affected.into_updates(), from_request: true });
        }
        self.rx.recv().await
    }

    #[cfg(test)]
    pub(crate) fn offline() -> UpdatesRx {
        UpdatesRx { rx: mpsc::channel(1).1, events: Arc::default() }
    }

    #[cfg(test)]
    pub(crate) fn channel() -> (mpsc::Sender<Incoming>, UpdatesRx) {
        let (tx, rx) = mpsc::channel(8);
        (tx, UpdatesRx { rx, events: Arc::default() })
    }
}

#[derive(Clone)]
pub struct Session {
    requests: mpsc::UnboundedSender<Request>,
    dc_id: i32,
    auth_key: [u8; 256],
    options: Arc<Options>,
}

impl Session {
    pub async fn connect(
        dc: Dc,
        auth_key: Option<[u8; 256]>,
        options: &Options,
        events: Arc<Events>,
    ) -> Result<(Session, UpdatesRx)> {
        let (tx, rx) = mpsc::channel(UPDATES_BUFFER);
        let session = Session::open(dc, auth_key, None, options, events.clone(), Some(tx)).await?;
        Ok((session, UpdatesRx { rx, events }))
    }

    #[cfg(test)]
    fn local(requests: mpsc::UnboundedSender<Request>) -> Session {
        Session { requests, dc_id: 2, auth_key: [0; 256], options: Arc::default() }
    }

    #[cfg(test)]
    pub(crate) fn set_options(&mut self, options: Options) {
        self.options = Arc::new(options);
    }

    #[cfg(test)]
    pub(crate) fn offline() -> Session {
        Session::local(mpsc::unbounded_channel().0)
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
        (Session::local(requests), sent)
    }

    /// A connection for file parts only. The server keeps initConnection's client info with the auth key
    /// (https://core.telegram.org/api/invoking#saving-client-info), so an authorized key needs no second one.
    pub(crate) async fn connect_transfer(
        dc: Dc,
        auth_key: Option<[u8; 256]>,
        cdn_key: Option<&PublicKey>,
        options: &Options,
        events: Arc<Events>,
    ) -> Result<Session> {
        Session::open(dc, auth_key, cdn_key, options, events, None).await
    }

    async fn open(
        dc: Dc,
        auth_key: Option<[u8; 256]>,
        cdn_key: Option<&PublicKey>,
        options: &Options,
        events: Arc<Events>,
        updates: Option<mpsc::Sender<Incoming>>,
    ) -> Result<Session> {
        let (conn, auth_key, salt, time_offset) = handshake(dc, auth_key, cdn_key, options).await?;
        let (requests_tx, requests) = mpsc::unbounded_channel();
        let options = Arc::new(options.clone());
        let runner = Runner::new(dc, options.clone(), conn, (auth_key, salt, time_offset), events, updates);
        tokio::spawn(runner.run(requests));
        Ok(Session { requests: requests_tx, dc_id: dc.id, auth_key, options })
    }

    pub fn dc_id(&self) -> i32 {
        self.dc_id
    }

    pub fn auth_key(&self) -> &[u8; 256] {
        &self.auth_key
    }

    pub(crate) fn options(&self) -> &Options {
        &self.options
    }

    pub(crate) fn downgrade(&self) -> WeakSession {
        let requests = self.requests.downgrade();
        WeakSession { requests, dc_id: self.dc_id, auth_key: self.auth_key, options: self.options.clone() }
    }

    /// How long to sleep off `error` before retrying: only FLOOD_WAITs within `flood_sleep_threshold`, each
    /// shortened to `cap` first.
    pub(crate) fn flood_sleep(&self, error: &Error, cap: Option<Duration>) -> Option<Duration> {
        let Error::Rpc(RpcError { code: 420, value: Some(wait), .. }) = error else { return None };
        let wait = Duration::from_secs((*wait).max(1).into());
        let wait = cap.map_or(wait, |cap| wait.min(cap));
        (wait <= self.options.flood_sleep_threshold).then_some(wait)
    }

    /// Waits up to the `request_timeout` the session was connected with. Dropping the future abandons the request.
    pub async fn invoke<R: RemoteCall>(&self, request: &R) -> Result<R::Return> {
        self.invoke_with_timeout(request, self.options.request_timeout).await
    }

    /// A request that times out is forgotten: the server is asked not to send its answer, and the connection
    /// carries on.
    pub async fn invoke_with_timeout<R: RemoteCall>(&self, request: &R, timeout: Duration) -> Result<R::Return> {
        let (answer, rx) = oneshot::channel();
        let request = Request { name: R::NAME, body: request.to_bytes(), answer };
        self.requests.send(request).map_err(|_| Error::Disconnected)?;
        let answer = tokio::time::timeout(timeout, rx).await.map_err(|_| Error::Timeout)?;
        let body = answer.map_err(|_| Error::Disconnected)??;
        Ok(R::Return::from_bytes(&body)?)
    }
}

pub(crate) struct WeakSession {
    requests: mpsc::WeakUnboundedSender<Request>,
    dc_id: i32,
    auth_key: [u8; 256],
    options: Arc<Options>,
}

impl WeakSession {
    pub(crate) fn upgrade(&self) -> Option<Session> {
        let requests = self.requests.upgrade().filter(|r| !r.is_closed())?;
        Some(Session { requests, dc_id: self.dc_id, auth_key: self.auth_key, options: self.options.clone() })
    }
}

pub(crate) fn init_connection<X>(
    api_id: i32,
    options: &Options,
    query: X,
) -> functions::InvokeWithLayer<functions::InitConnection<X>> {
    functions::InvokeWithLayer {
        layer: LAYER,
        query: functions::InitConnection {
            api_id,
            device_model: "vaegram".into(),
            system_version: std::env::consts::OS.into(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            system_lang_code: "en".into(),
            lang_pack: String::new(),
            lang_code: "en".into(),
            proxy: options.proxy.as_ref().and_then(Proxy::input_client_proxy),
            params: None,
            query,
        },
    }
}

async fn handshake(
    dc: Dc,
    auth_key: Option<[u8; 256]>,
    cdn_key: Option<&PublicKey>,
    options: &Options,
) -> Result<(Connection, [u8; 256], i64, Option<i64>)> {
    Ok(match auth_key {
        // A stored key brings no server time; the offset is measured once the server complains.
        Some(key) => (Connection::connect(&dc, options).await?, key, 0, None),
        None => {
            let key = match cdn_key {
                Some(key) => key.clone(),
                None if dc.test => PublicKey::test(),
                None => PublicKey::production(),
            };
            let mut attempt = 1;
            loop {
                let mut conn = Connection::connect(&dc, options).await?;
                match auth::create(&mut conn, dc, &key).await {
                    Ok(c) => break (conn, c.auth_key, c.salt, Some(c.time_offset)),
                    Err(e) if attempt == HANDSHAKE_ATTEMPTS => return Err(e),
                    Err(_) => attempt += 1,
                }
            }
        }
    })
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
    name: &'static str,
    body: Vec<u8>,
    answer: oneshot::Sender<Answer>,
    // Kept across re-sends, so the observer sees the whole wait.
    start: Instant,
    container: i64,
    // The server confirmed receipt; after a reconnect it re-sends the answer by itself.
    acked: bool,
}

struct Runner {
    dc: Dc,
    options: Arc<Options>,
    writer: Writer,
    packets: mpsc::Receiver<Result<Vec<u8>>>,
    reader: JoinHandle<()>,
    auth_key: [u8; 256],
    auth_key_id: [u8; 8],
    session_id: [u8; 8],
    salt: i64,
    msg_ids: MsgId,
    clock_synced: bool,
    server_msg_ids: ServerMsgIds,
    seq: SeqNo,
    pending: HashMap<i64, Pending>,
    queue: VecDeque<Pending>,
    service: VecDeque<(Vec<u8>, bool)>,
    acks: Vec<i64>,
    buf: Vec<u8>,
    updates: Option<mpsc::Sender<Incoming>>,
    events: Arc<Events>,
}

impl Runner {
    fn new(
        dc: Dc,
        options: Arc<Options>,
        conn: Connection,
        (auth_key, salt, time_offset): ([u8; 256], i64, Option<i64>),
        events: Arc<Events>,
        updates: Option<mpsc::Sender<Incoming>>,
    ) -> Runner {
        let (writer, packets, reader) = start_reading(conn);
        Runner {
            dc,
            options,
            writer,
            packets,
            reader,
            auth_key,
            auth_key_id: sha1(&[&auth_key])[12..].try_into().unwrap(),
            session_id: random(),
            salt,
            msg_ids: MsgId::new(time_offset.unwrap_or(0)),
            clock_synced: time_offset.is_some(),
            server_msg_ids: ServerMsgIds::default(),
            seq: SeqNo::default(),
            pending: HashMap::new(),
            queue: VecDeque::new(),
            service: VecDeque::new(),
            acks: Vec::new(),
            buf: Vec::new(),
            updates,
            events,
        }
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
                    self.reconnect(&requests).await;
                    ping.reset();
                }
            }
        };
        self.reader.abort();
        let code = match reason {
            Error::Transport(code) => Some(code),
            _ => None,
        };
        let queued = std::mem::take(&mut self.queue).into_iter().map(|p| (0, p));
        for (msg_id, p) in std::mem::take(&mut self.pending).into_iter().chain(queued) {
            self.finish(msg_id, p, Err(code.map_or(Error::Disconnected, Error::Transport)));
        }
    }

    async fn reconnect(&mut self, requests: &mpsc::UnboundedReceiver<Request>) {
        let mut delay = RECONNECT_DELAY;
        let mut attempt = 0;
        while !requests.is_closed() && self.updates.as_ref().is_none_or(|u| !u.is_closed()) {
            attempt += 1;
            self.reader.abort();
            tokio::time::sleep(delay).await;
            delay = (delay * 2).min(MAX_RECONNECT_DELAY);
            self.drop_abandoned();
            let Ok(conn) = Connection::connect(&self.dc, &self.options).await else { continue };
            (self.writer, self.packets, self.reader) = start_reading(conn);
            if self.resend_pending().await.is_ok() {
                self.events.reconnected();
                self.events.emit(ClientEvent::Reconnected { dc: self.dc.id, attempt });
                self.request_difference();
                return;
            }
        }
    }

    fn request_difference(&self) {
        self.push(Incoming { updates: base::Updates::UpdatesTooLong, from_request: false });
    }

    fn deliver(&self, body: &[u8], from_request: bool) {
        if self.updates.is_none() {
            return;
        }
        match base::Updates::from_bytes(body) {
            Ok(updates) => self.push(Incoming { updates, from_request }),
            Err(e) if !from_request => self.skip(body, e),
            Err(_) => {}
        }
    }

    fn push(&self, incoming: Incoming) {
        let Some(tx) = &self.updates else { return };
        if let Err(mpsc::error::TrySendError::Full(_)) = tx.try_send(incoming) {
            self.events.drop_updates();
        }
    }

    fn skip(&self, body: &[u8], error: crate::raw::Error) {
        self.events.emit(match error {
            crate::raw::Error::UnexpectedConstructor { id } => ClientEvent::UnknownConstructor { id },
            _ => ClientEvent::UpdateParseFailed { constructor_id: constructor_id(body).unwrap_or(0), len: body.len() },
        });
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
        let Request { name, body, answer } = r;
        let p = Pending { name, body, answer, start: Instant::now(), container: 0, acked: false };
        if p.answer.is_closed() {
            self.finish(0, p, Err(Error::Timeout));
        } else {
            self.queue.push_back(p);
        }
    }

    // Queued ones (waiting out a reconnect) never went out, so they need no rpc_drop_answer.
    fn drop_abandoned(&mut self) {
        let gone: Vec<i64> = self.pending.iter().filter(|(_, p)| p.answer.is_closed()).map(|(&id, _)| id).collect();
        for req_msg_id in gone {
            if let Some(p) = self.pending.remove(&req_msg_id) {
                self.finish(req_msg_id, p, Err(Error::Timeout));
            }
            self.service.push_back((functions::RpcDropAnswer { req_msg_id }.to_bytes(), true));
        }
        if self.queue.iter().any(|p| p.answer.is_closed()) {
            let queue = std::mem::take(&mut self.queue);
            let (gone, queue): (VecDeque<_>, _) = queue.into_iter().partition(|p| p.answer.is_closed());
            self.queue = queue;
            for p in gone {
                self.finish(0, p, Err(Error::Timeout));
            }
        }
    }

    async fn ping(&mut self) -> Result<()> {
        let ping = functions::PingDelayDisconnect { ping_id: i64::from_le_bytes(random()), disconnect_delay: 75 };
        self.service.push_back((ping.to_bytes(), true));
        self.flush().await
    }

    fn answer(&mut self, msg_id: i64, answer: Answer) {
        if let Some(p) = self.pending.remove(&msg_id) {
            self.finish(msg_id, p, answer);
        }
    }

    fn finish(&self, msg_id: i64, p: Pending, answer: Answer) {
        if let Some(Observer(observe)) = &self.options.observer {
            let timeout = Error::Timeout;
            let result = if p.answer.is_closed() { Err(&timeout) } else { answer.as_deref() };
            let call = Call { name: p.name, msg_id, request: &p.body, duration: p.start.elapsed(), result };
            // A panicking observer would otherwise take the connection down with it.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(&call)));
        }
        let _ = p.answer.send(answer);
    }

    fn take_sent(&mut self, msg_id: i64) -> Vec<(i64, Pending)> {
        if let Some(p) = self.pending.remove(&msg_id) {
            return vec![(msg_id, p)];
        }
        let ids: Vec<i64> = self.pending.iter().filter(|(_, p)| p.container == msg_id).map(|(&id, _)| id).collect();
        ids.into_iter().filter_map(|id| Some((id, self.pending.remove(&id)?))).collect()
    }

    fn resend(&mut self, msg_id: i64) {
        let sent = self.take_sent(msg_id);
        self.queue.extend(sent.into_iter().map(|(_, p)| p));
    }

    async fn handle_packet(&mut self, packet: &[u8]) -> Result<()> {
        // Drop anything that fails decryption or the MTProto checks: it isn't from the server.
        let Ok(msg) = mtproto::unpack(packet, &self.auth_key, &self.auth_key_id, &self.session_id, true) else {
            self.events.emit(ClientEvent::PacketDropped { reason: "failed decryption or the MTProto checks" });
            return Ok(());
        };
        if !self.fresh(msg.msg_id, &msg.body) {
            return Ok(());
        }
        if constructor_id(&msg.body) == Some(msg_container::ID) {
            match msg_container::parse(&msg.body) {
                Ok(messages) => {
                    for m in messages {
                        if self.fresh(m.msg_id, m.body) {
                            self.handle_message(m.msg_id, m.seq_no, m.body);
                        }
                    }
                }
                Err(_) => self.events.emit(ClientEvent::PacketDropped { reason: "bad msg_container" }),
            }
        } else {
            self.handle_message(msg.msg_id, msg.seq_no, &msg.body);
        }
        if self.acks.len() >= ACKS_THRESHOLD || !self.queue.is_empty() || !self.service.is_empty() {
            self.flush().await?;
        }
        Ok(())
    }

    // Server msg_ids must be odd, recent and never seen before; anything else is dropped as a
    // possible replay. The time window waits for a measured clock offset, and clock-sync
    // notifications skip it, since a wrong clock is exactly what they fix.
    fn fresh(&mut self, msg_id: i64, body: &[u8]) -> bool {
        if !self.clock_synced && ServerMsgIds::in_window(msg_id, internals::now() + self.msg_ids.time_offset) {
            self.clock_synced = true;
        }
        let now = (self.clock_synced && constructor_id(body) != Some(BAD_MSG_NOTIFICATION))
            .then(|| internals::now() + self.msg_ids.time_offset);
        msg_id % 2 != 0 && self.server_msg_ids.accept(msg_id, now)
    }

    fn handle_message(&mut self, msg_id: i64, seq_no: i32, body: &[u8]) {
        if seq_no % 2 == 1 {
            self.acks.push(msg_id);
        }
        let unpacked;
        let body = if constructor_id(body) == Some(gzip_packed::ID) {
            let Ok(b) = gzip_packed::unpack(body) else {
                return self.events.emit(ClientEvent::PacketDropped { reason: "bad gzip_packed" });
            };
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
            _ => self.deliver(body, false),
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
            self.deliver(&result, true);
            Ok(result)
        };
        self.answer(req_msg_id, answer);
    }
}
