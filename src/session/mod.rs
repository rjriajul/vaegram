mod auth;
pub(crate) mod cdn;
mod events;
pub(crate) mod internals;
pub(crate) mod no_updates;
mod outbox;
mod pfs;
mod rate_limiter;
mod replay;
mod retry;
mod service;
#[cfg(test)]
pub(crate) mod test_dc;
#[cfg(test)]
mod tests;
pub(crate) mod transfer;

pub(crate) use events::Affected;
#[cfg(feature = "unstable-raw")]
pub use events::Events;
#[cfg(not(feature = "unstable-raw"))]
pub(crate) use events::Events;
pub use internals::data_center::Dc;
pub use rate_limiter::{RateCategory, RateLimiter, RateLimits};
pub use replay::Replay;
pub use retry::{AutoSleep, NoRetries, Retry, RetryContext, RetryPolicy};

use std::collections::{HashMap, VecDeque};
use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::{Duration, Instant};

use tokio::sync::{Notify, mpsc, oneshot};

use crate::connection::{Connection, Device, Options, Proxy, Reader, Writer};
use crate::crypto::rsa::PublicKey;
use crate::crypto::{mtproto, random};
use crate::enums::ClientEvent;
use crate::errors::{Error, Result, RpcError};
use crate::raw::core::{gzip_packed, msg_container};
use crate::raw::{Deserializable, Identifiable, LAYER, RemoteCall, Serializable, base, functions, types};
use crate::types::Json;
use internals::{MsgId, SeqNo, ServerMsgIds};
use outbox::{MAX_CONTAINER_MSGS, MAX_IDS};
use pfs::Pfs;

const RPC_RESULT: u32 = 0xf35c6d01;
const RPC_ERROR: u32 = <types::RpcError as Identifiable>::CONSTRUCTOR_ID;
const BAD_SERVER_SALT: u32 = <types::BadServerSalt as Identifiable>::CONSTRUCTOR_ID;
const BAD_MSG_NOTIFICATION: u32 = <types::BadMsgNotification as Identifiable>::CONSTRUCTOR_ID;
const NEW_SESSION_CREATED: u32 = <types::NewSessionCreated as Identifiable>::CONSTRUCTOR_ID;
const PONG: u32 = <types::Pong as Identifiable>::CONSTRUCTOR_ID;
const MSGS_STATE_REQ: u32 = <types::MsgsStateReq as Identifiable>::CONSTRUCTOR_ID;
const MSGS_ALL_INFO: u32 = <types::MsgsAllInfo as Identifiable>::CONSTRUCTOR_ID;
const MSGS_STATE_INFO: u32 = <types::MsgsStateInfo as Identifiable>::CONSTRUCTOR_ID;
const MSG_DETAILED_INFO: u32 = <types::MsgDetailedInfo as Identifiable>::CONSTRUCTOR_ID;
const MSG_NEW_DETAILED_INFO: u32 = <types::MsgNewDetailedInfo as Identifiable>::CONSTRUCTOR_ID;
const MSG_RESEND_REQ: u32 = <types::MsgResendReq as Identifiable>::CONSTRUCTOR_ID;
const MSGS_ACK: u32 = <types::MsgsAck as Identifiable>::CONSTRUCTOR_ID;

const HANDSHAKE_ATTEMPTS: usize = 3;
const PING_EVERY: Duration = Duration::from_secs(60);
// No reads at all, not even a pong, for this long: the link died without the OS noticing.
const DEAD_AFTER: Duration = Duration::from_secs(75);
// With requests waiting, silence gets a ping sooner and a shorter deadline, so a dead NAT mapping doesn't
// stall them for DEAD_AFTER.
const PROBE_AFTER: Duration = Duration::from_secs(10);
const PROBE_DEAD_AFTER: Duration = Duration::from_secs(20);
const ACKS_THRESHOLD: usize = 10;
const RECONNECT_DELAY: Duration = Duration::from_millis(500);
const MAX_RECONNECT_DELAY: Duration = Duration::from_secs(30);
// A server that accepts the connection but never finishes the key exchange.
const KEY_EXCHANGE_TIMEOUT: Duration = Duration::from_secs(20);
const TICK: Duration = Duration::from_secs(1);
const ACK_EVERY: Duration = Duration::from_secs(15);

type Answer = Result<Vec<u8>>;

/// The body and tag of each request a scripted test session took.
#[cfg(test)]
pub(crate) type TaggedBodies = mpsc::UnboundedReceiver<(Vec<u8>, Option<u64>)>;

struct Request {
    name: &'static str,
    body: Vec<u8>,
    tag: Option<u64>,
    answer: oneshot::Sender<Answer>,
}

unstable_raw_pub! {
    /// One push from the server, at the session level, where requests and answers are raw TL too.
    ///
    /// Experimental (`unstable-raw`): may change or go away in any release.
    struct Incoming {
        pub(crate) updates: base::Updates,
        pub(crate) from_request: bool,
    }
}

impl Incoming {
    /// Experimental (`unstable-raw`): may change or go away in any release.
    #[cfg(feature = "unstable-raw")]
    pub fn into_updates(self) -> base::Updates {
        self.updates
    }

    /// Sent back as the answer to one of this session's own requests, not pushed.
    #[cfg(feature = "unstable-raw")]
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
    /// it as is. A recording keeps the redacted form; `request_json` (with the `json` feature) gives it as JSON.
    pub request: &'a [u8],
    /// From when the connection's task took the request in to its end, re-sends included.
    pub duration: Duration,
    /// The answer's TL bytes, or the error the caller got. A caller that stopped waiting got `Error::Timeout`.
    pub result: Result<&'a [u8], &'a Error>,
    /// The data center the request went to.
    pub dc: i32,
    /// 0 for the main connection; file transfers number theirs per DC, from 1.
    pub connection: u32,
    /// Times it went out again: after a bad_server_salt, a bad_msg_notification or a lost connection.
    pub resent: u16,
    /// The tag of the innermost [`tagged`](crate::observe::tagged) the request was sent inside, read on the
    /// sending task when it sent it; `None` for a request sent outside one, and for what vaegram sends on its own
    /// (pings, the dispatcher's `getDifference`, the background loop of a repeated chat action). Requests a vaegram
    /// method sends from a task it starts for that call carry the caller's tag, and a re-sent request keeps its own.
    pub tag: Option<u64>,
}

#[cfg(feature = "json")]
impl Call<'_> {
    /// [`request`](Self::request) as JSON in the shape of [`ToJson`](crate::raw::ToJson), with credentials cut as
    /// a [recording](crate::storage::recording) cuts them: a call that carries one comes out as its outermost name
    /// only, such as `{"cut":"auth.signIn"}`, or `{"cut":"invokeWithLayer"}` when it went out wrapped.
    pub fn request_json(&self) -> String {
        crate::storage::recording::request_json(self.request)
    }

    /// The answer as JSON in the shape of [`ToJson`](crate::raw::ToJson), read as the type the request returns, so
    /// a bare vector such as the answer to `users.getUsers` comes out whole. As in a
    /// [recording](crate::storage::recording), the answer to a request that was cut is cut too, as is one that holds
    /// a credential itself: only its name is left, such as `{"cut":"auth.authorization"}`. `None` when
    /// [`result`](Self::result) is an error; the error itself carries no credentials.
    pub fn result_json(&self) -> Option<String> {
        self.result.ok().map(|answer| crate::storage::recording::answer_json(self.request, answer))
    }
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

unstable_raw_pub! {
    /// The updates a main session receives, in arrival order, for the dispatcher. It holds
    /// [`Options::updates_capacity`](crate::connection::Options::updates_capacity) batches; past that, batches are
    /// dropped and an `UpdatesTooLong` stands in for them.
    ///
    /// Experimental (`unstable-raw`): may change or go away in any release.
    struct UpdatesRx {
        rx: mpsc::Receiver<Incoming>,
        pub(crate) events: Arc<Events>,
    }
}

impl UpdatesRx {
    /// The next batch of updates; `None` once the session's task has ended. Updates that were dropped because the
    /// queue was full come back as one `UpdatesTooLong`, after `ClientEvent::UpdatesDropped` fires.
    pub async fn recv(&mut self) -> Option<Incoming> {
        loop {
            let (count, unrecoverable) = self.events.take_dropped();
            if count > 0 {
                self.events.emit(ClientEvent::UpdatesDropped { count, unrecoverable });
                return Some(Incoming { updates: base::Updates::UpdatesTooLong, from_request: false });
            }
            if let Some(affected) = self.events.take_affected() {
                return Some(Incoming { updates: affected.into_updates(), from_request: true });
            }
            if let Some(updates) = self.events.take_nested() {
                return Some(Incoming { updates, from_request: true });
            }
            tokio::select! {
                incoming = self.rx.recv() => return incoming,
                () = self.events.wake.notified() => {}
            }
        }
    }

    #[cfg(test)]
    pub(crate) fn offline() -> UpdatesRx {
        UpdatesRx { rx: mpsc::channel(1).1, events: Arc::default() }
    }

    #[cfg(test)]
    pub(crate) fn channel() -> (mpsc::Sender<Incoming>, UpdatesRx) {
        UpdatesRx::channel_with(Arc::default())
    }

    #[cfg(test)]
    pub(crate) fn channel_with(events: Arc<Events>) -> (mpsc::Sender<Incoming>, UpdatesRx) {
        let (tx, rx) = mpsc::channel(8);
        (tx, UpdatesRx { rx, events })
    }
}

unstable_raw_pub! {
    /// One MTProto session with a data center: an auth key and an encrypted connection that a background task owns.
    /// The task pings, reconnects with backoff after a failure and sends the waiting requests again. Clones share
    /// the connection; it closes once every clone is dropped.
    ///
    /// Experimental (`unstable-raw`): may change or go away in any release.
    #[derive(Clone)]
    struct Session {
        requests: mpsc::UnboundedSender<Request>,
        dc_id: i32,
        auth_key: [u8; 256],
        // When this process made the permanent key; a bind under a newer one may be refused while it propagates.
        key_made: Option<tokio::time::Instant>,
        options: Arc<Options>,
        pfs: Option<Arc<[u8]>>,
        learned: Arc<Learned>,
        control: Arc<Control>,
    }
}

/// The server salt and the clock offset, saved so a restart skips the first bad_server_salt and starts with
/// msg_ids the server accepts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub struct Salt {
    /// The salt Telegram last gave this session, sent in every encrypted message.
    pub value: i64,
    /// Server time minus local time, in seconds.
    pub time_offset: i64,
}

impl Salt {
    /// A server salt and the clock offset (server time minus local time, in seconds) it was learned with.
    pub fn new(value: i64, time_offset: i64) -> Self {
        Salt { value, time_offset }
    }
}

/// A permanent key this process already has.
#[derive(Clone, Copy)]
pub(crate) struct Known {
    pub(crate) key: [u8; 256],
    pub(crate) salt: Salt,
    /// When this process made it; `None` for a stored key.
    pub(crate) made: Option<tokio::time::Instant>,
}

#[derive(Default)]
struct Learned {
    salt: AtomicI64,
    time_offset: AtomicI64,
}

// Close and restart are signals to the task that owns the connection, so it never races another owner.
#[derive(Default)]
struct Control {
    closed: AtomicBool,
    restart: AtomicBool,
    // Whether the task has a working connection: set as the link goes down and comes back, cleared when it ends.
    connected: AtomicBool,
    // notify_one keeps a permit, so a runner busy elsewhere still sees the signal on its next wait.
    wake: Notify,
}

impl Control {
    fn close(&self) {
        self.closed.store(true, Ordering::Release);
        self.wake.notify_one();
    }

    fn closed(&self) -> bool {
        self.closed.load(Ordering::Acquire)
    }
}

impl Session {
    /// `auth_key` is a stored key with the salt last saved for it; `None` makes a new one. With `pfs`, the
    /// traffic runs under a temporary key bound to it, and `pfs` is the request sent right after each bind.
    pub async fn connect(
        dc: Dc,
        auth_key: Option<([u8; 256], Salt)>,
        options: &Options,
        events: Arc<Events>,
        pfs: Option<Vec<u8>>,
    ) -> Result<(Session, UpdatesRx)> {
        let (tx, rx) = mpsc::channel(options.updates_capacity.max(1));
        let pfs = pfs.map(Arc::from);
        let auth_key = auth_key.map(|(key, salt)| Known { key, salt, made: None });
        let session = Session::open(dc, auth_key, None, options, events.clone(), Role::Main(tx), pfs).await?;
        Ok((session, UpdatesRx { rx, events }))
    }

    #[cfg(test)]
    fn local(requests: mpsc::UnboundedSender<Request>) -> Session {
        let (options, learned, control) = (Arc::default(), Arc::default(), Arc::default());
        Session { requests, dc_id: 2, auth_key: [0; 256], key_made: None, options, pfs: None, learned, control }
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
    pub(crate) fn scripted(replies: Vec<Result<Vec<u8>>>) -> (Session, mpsc::UnboundedReceiver<Vec<u8>>) {
        Session::scripted_as(replies, |r| std::mem::take(&mut r.body))
    }

    /// [`scripted`](Self::scripted), handing back each request's tag along with its body.
    #[cfg(test)]
    pub(crate) fn scripted_tags(replies: Vec<Result<Vec<u8>>>) -> (Session, TaggedBodies) {
        Session::scripted_as(replies, |r| (std::mem::take(&mut r.body), r.tag))
    }

    #[cfg(test)]
    fn scripted_as<T: Send + 'static>(
        replies: Vec<Result<Vec<u8>>>,
        seen: fn(&mut Request) -> T,
    ) -> (Session, mpsc::UnboundedReceiver<T>) {
        let (requests, mut rx) = mpsc::unbounded_channel::<Request>();
        let (log, sent) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut replies = replies.into_iter();
            while let Some(mut request) = rx.recv().await {
                let _ = log.send(seen(&mut request));
                if let Some(reply) = replies.next() {
                    let _ = request.answer.send(reply);
                }
            }
        });
        (Session::local(requests), sent)
    }

    /// A session that takes requests and never answers them.
    #[cfg(test)]
    pub(crate) fn silent() -> (Session, mpsc::UnboundedReceiver<Vec<u8>>) {
        let (requests, mut rx) = mpsc::unbounded_channel::<Request>();
        let (bodies, sent) = mpsc::unbounded_channel();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Some(request) = rx.recv().await {
                let _ = bodies.send(request.body);
                held.push(request.answer);
            }
        });
        (Session::local(requests), sent)
    }

    /// A connection for file parts only. The server keeps initConnection's client info with the auth key
    /// (https://core.telegram.org/api/invoking#saving-client-info), so an authorized key needs no second one,
    /// unless `pfs` puts it under a temporary key.
    pub(crate) async fn connect_transfer(
        dc: Dc,
        auth_key: Option<Known>,
        cdn_key: Option<&PublicKey>,
        options: &Options,
        events: Arc<Events>,
        pfs: Option<Arc<[u8]>>,
        connection: u32,
    ) -> Result<Session> {
        Session::open(dc, auth_key, cdn_key, options, events, Role::Transfer(connection), pfs).await
    }

    async fn open(
        dc: Dc,
        auth_key: Option<Known>,
        cdn_key: Option<&PublicKey>,
        options: &Options,
        events: Arc<Events>,
        role: Role,
        pfs: Option<Arc<[u8]>>,
    ) -> Result<Session> {
        let (updates, connection) = match role {
            Role::Main(updates) => (Some(updates), 0),
            Role::Transfer(connection) => (None, connection),
        };
        let key_made = auth_key.map_or_else(|| Some(tokio::time::Instant::now()), |k| k.made);
        let (mut conn, auth_key, salt, clock_synced) =
            handshake(dc, auth_key.map(|k| (k.key, k.salt)), cdn_key, options).await?;
        let options = Arc::new(options.clone());
        let mut runner = match &pfs {
            None => Runner::new(dc, options.clone(), conn, (auth_key, salt, clock_synced), events, updates),
            Some(init) => {
                let temp = create_key(&mut conn, dc, &server_key(dc), Some(pfs::TEMP_KEY_LIFETIME)).await?;
                let expires_at = internals::now() + temp.time_offset + pfs::TEMP_KEY_LIFETIME as i64;
                let key = (temp.auth_key, Salt { value: temp.salt, time_offset: temp.time_offset }, true);
                let mut runner = Runner::new(dc, options.clone(), conn, key, events, updates);
                runner.pfs = Some(Pfs::new(auth_key, key_made, init.to_vec(), expires_at));
                runner
            }
        };
        runner.connection = connection;
        let (learned, control) = (runner.learned.clone(), runner.control.clone());
        let (requests_tx, requests) = mpsc::unbounded_channel();
        tokio::spawn(runner.run(requests));
        Ok(Session { requests: requests_tx, dc_id: dc.id, auth_key, key_made, options, pfs, learned, control })
    }

    /// Ends the connection for good: waiting and later requests fail with `Error::Disconnected`.
    pub(crate) fn close(&self) {
        self.control.close();
    }

    /// Drops the connection and reconnects at once, the way a lost one comes back.
    pub(crate) fn restart(&self) -> Result<()> {
        if self.requests.is_closed() || self.control.closed() {
            return Err(Error::Disconnected);
        }
        self.control.restart.store(true, Ordering::Release);
        self.control.wake.notify_one();
        Ok(())
    }

    /// Whether the connection is up: `false` while it reconnects and once it is closed or ended.
    pub(crate) fn is_connected(&self) -> bool {
        !self.control.closed() && self.control.connected.load(Ordering::Acquire)
    }

    /// The key, with its salt and when this process made it, for another connection on it.
    pub(crate) fn known_key(&self) -> Known {
        Known { key: self.auth_key, salt: self.salt(), made: self.key_made }
    }

    /// The data center this session is connected to. It never changes: moving to another data center takes a new
    /// session.
    pub fn dc_id(&self) -> i32 {
        self.dc_id
    }

    /// The permanent auth key: the login itself, so keep it secret. With PFS, traffic runs under a temporary key
    /// bound to this one.
    pub fn auth_key(&self) -> &[u8; 256] {
        &self.auth_key
    }

    pub(crate) fn options(&self) -> &Options {
        &self.options
    }

    pub(crate) fn pfs(&self) -> bool {
        self.pfs.is_some()
    }

    pub(crate) fn downgrade(&self) -> WeakSession {
        let (requests, options, pfs) = (self.requests.downgrade(), self.options.clone(), self.pfs.clone());
        let (learned, control, key_made) = (self.learned.clone(), self.control.clone(), self.key_made);
        let (dc_id, auth_key) = (self.dc_id, self.auth_key);
        WeakSession { requests, dc_id, auth_key, key_made, options, pfs, learned, control }
    }

    /// What the retry policy says about a failed request: `Continue(wait)` to sleep `wait` and send it again. A
    /// FLOOD_WAIT reaches the policy shortened to `cap` and at least a second.
    pub(crate) fn should_retry(
        &self,
        request: &'static str,
        error: &Error,
        fail_count: u32,
        slept_so_far: Duration,
        cap: Option<Duration>,
    ) -> ControlFlow<(), Duration> {
        let flood_wait = error.flood_wait().map(|wait| wait.max(Duration::from_secs(1)));
        let flood_wait = flood_wait.map(|wait| cap.map_or(wait, |cap| wait.min(cap)));
        let ctx = RetryContext { request, error, fail_count, slept_so_far, flood_wait };
        self.options.retry.0.should_retry(&ctx)
    }

    /// The salt and clock offset the connection last learned, to save for the next start.
    pub fn salt(&self) -> Salt {
        let l = &self.learned;
        Salt { value: l.salt.load(Ordering::Relaxed), time_offset: l.time_offset.load(Ordering::Relaxed) }
    }

    /// Waits up to the `request_timeout` the session was connected with. Dropping the future abandons the request.
    pub async fn invoke<R: RemoteCall>(&self, request: &R) -> Result<R::Return> {
        self.invoke_with_timeout(request, self.options.request_timeout).await
    }

    /// A request that times out is forgotten: the server is asked not to send its answer, and the connection
    /// carries on.
    pub async fn invoke_with_timeout<R: RemoteCall>(&self, request: &R, timeout: Duration) -> Result<R::Return> {
        let (answer, rx) = oneshot::channel();
        let request = Request { name: R::NAME, body: request.to_bytes(), tag: crate::observe::current_tag(), answer };
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
    key_made: Option<tokio::time::Instant>,
    options: Arc<Options>,
    pfs: Option<Arc<[u8]>>,
    learned: Arc<Learned>,
    control: Arc<Control>,
}

impl WeakSession {
    pub(crate) fn upgrade(&self) -> Option<Session> {
        let requests = self.requests.upgrade().filter(|r| !r.is_closed())?;
        let (options, pfs, learned) = (self.options.clone(), self.pfs.clone(), self.learned.clone());
        let (dc_id, auth_key, key_made, control) = (self.dc_id, self.auth_key, self.key_made, self.control.clone());
        Some(Session { requests, dc_id, auth_key, key_made, options, pfs, learned, control })
    }
}

pub(crate) fn init_connection<X>(
    api_id: i32,
    options: &Options,
    query: X,
) -> functions::InvokeWithLayer<functions::InitConnection<X>> {
    let device = &options.device;
    let params = init_params(device);
    functions::InvokeWithLayer {
        layer: LAYER,
        query: functions::InitConnection {
            api_id,
            device_model: device.model.clone(),
            system_version: device.system_version.clone(),
            app_version: device.app_version.clone(),
            system_lang_code: device.system_lang_code.as_ref().unwrap_or(&device.lang_code).clone(),
            lang_pack: device.lang_pack.clone(),
            lang_code: device.lang_code.clone(),
            proxy: options.proxy.as_ref().and_then(Proxy::input_client_proxy),
            params,
            query,
        },
    }
}

/// What `initConnection.params` carries. A `tz_offset` replaces the same key of an object or is appended to it;
/// anything that isn't an object goes out as given, which `ClientBuilder` refuses to combine with a `tz_offset`.
fn init_params(device: &Device) -> Option<base::JSONValue> {
    let tz_offset = device.tz_offset.map(|offset| Json::Number(f64::from(offset)));
    match (device.params.as_ref(), tz_offset) {
        (None, None) => None,
        (None, Some(offset)) => Some(json_value(&Json::Object(vec![("tz_offset".into(), offset)]))),
        (Some(Json::Object(fields)), Some(offset)) => {
            let mut fields = fields.clone();
            match fields.iter_mut().find(|(key, _)| key == "tz_offset") {
                Some(field) => field.1 = offset,
                None => fields.push(("tz_offset".into(), offset)),
            }
            Some(json_value(&Json::Object(fields)))
        }
        (Some(params), _) => Some(json_value(params)),
    }
}

fn json_value(json: &Json) -> base::JSONValue {
    use base::JSONValue as J;
    match json {
        Json::Null => J::JsonNull,
        Json::Bool(value) => J::JsonBool(types::JsonBool { value: *value }),
        Json::Number(value) => J::JsonNumber(types::JsonNumber { value: *value }),
        Json::String(value) => J::JsonString(Box::new(types::JsonString { value: value.clone() })),
        Json::Array(items) => {
            J::JsonArray(Box::new(types::JsonArray { value: items.iter().map(json_value).collect() }))
        }
        Json::Object(fields) => {
            let field = |(key, value): &(String, Json)| {
                let entry = types::JsonObjectValue { key: key.clone(), value: json_value(value) };
                base::JSONObjectValue::JsonObjectValue(Box::new(entry))
            };
            J::JsonObject(Box::new(types::JsonObject { value: fields.iter().map(field).collect() }))
        }
    }
}

fn server_key(dc: Dc) -> PublicKey {
    #[cfg(test)]
    if dc.addr.ip().is_loopback() {
        return test_dc::public_key();
    }
    if dc.test { PublicKey::test() } else { PublicKey::production() }
}

async fn create_key(conn: &mut Connection, dc: Dc, key: &PublicKey, expires_in: Option<i32>) -> Result<auth::Created> {
    let timed_out = || std::io::Error::new(std::io::ErrorKind::TimedOut, "the key exchange timed out");
    tokio::time::timeout(KEY_EXCHANGE_TIMEOUT, auth::create(conn, dc, key, expires_in))
        .await
        .map_err(|_| timed_out())?
}

async fn handshake(
    dc: Dc,
    auth_key: Option<([u8; 256], Salt)>,
    cdn_key: Option<&PublicKey>,
    options: &Options,
) -> Result<(Connection, [u8; 256], Salt, bool)> {
    Ok(match auth_key {
        // A stored key brings no server time: the saved offset stands until the server confirms or corrects it.
        Some((key, salt)) => (Connection::connect(&dc, options).await?, key, salt, false),
        None => {
            let key = cdn_key.cloned().unwrap_or_else(|| server_key(dc));
            let mut attempt = 1;
            loop {
                let mut conn = Connection::connect(&dc, options).await?;
                match create_key(&mut conn, dc, &key, None).await {
                    Ok(c) => break (conn, c.auth_key, Salt { value: c.salt, time_offset: c.time_offset }, true),
                    Err(e) if attempt == HANDSHAKE_ATTEMPTS => return Err(e),
                    Err(_) => attempt += 1,
                }
            }
        }
    })
}

fn constructor_id(body: &[u8]) -> Option<u32> {
    Some(u32::from_le_bytes(body.get(..4)?.try_into().unwrap()))
}

// The main connection gets updates; transfer ones are numbered per DC.
enum Role {
    Main(mpsc::Sender<Incoming>),
    Transfer(u32),
}

struct Pending {
    name: &'static str,
    body: Vec<u8>,
    tag: Option<u64>,
    answer: oneshot::Sender<Answer>,
    // Kept across re-sends, so the observer sees the whole wait.
    start: Instant,
    container: i64,
    // The server confirmed receipt; after a reconnect it re-sends the answer by itself.
    acked: bool,
    resent: u16,
}

struct Runner {
    dc: Dc,
    connection: u32,
    options: Arc<Options>,
    reader: Reader,
    writer: Writer,
    auth_key: [u8; 256],
    auth_key_id: [u8; 8],
    session_id: [u8; 8],
    learned: Arc<Learned>,
    msg_ids: MsgId,
    clock_synced: bool,
    server_msg_ids: ServerMsgIds,
    seq: SeqNo,
    pending: HashMap<i64, Pending>,
    checking: HashMap<i64, Vec<i64>>,
    recheck: Vec<i64>,
    queue: VecDeque<Pending>,
    service: VecDeque<(Vec<u8>, bool)>,
    acks: Vec<i64>,
    updates: Option<mpsc::Sender<Incoming>>,
    events: Arc<Events>,
    pfs: Option<Pfs>,
    control: Arc<Control>,
    // What `on_link` last reported for the main connection.
    link_up: bool,
    last_read: tokio::time::Instant,
    probed: bool,
}

impl Runner {
    fn new(
        dc: Dc,
        options: Arc<Options>,
        conn: Connection,
        (auth_key, salt, clock_synced): ([u8; 256], Salt, bool),
        events: Arc<Events>,
        updates: Option<mpsc::Sender<Incoming>>,
    ) -> Runner {
        let (reader, writer) = conn.into_split();
        Runner {
            dc,
            connection: 0,
            options,
            reader,
            writer,
            auth_key,
            auth_key_id: mtproto::auth_key_id(&auth_key),
            session_id: random(),
            learned: Arc::new(Learned { salt: salt.value.into(), time_offset: salt.time_offset.into() }),
            msg_ids: MsgId::new(salt.time_offset),
            clock_synced,
            server_msg_ids: ServerMsgIds::default(),
            seq: SeqNo::default(),
            pending: HashMap::new(),
            checking: HashMap::new(),
            recheck: Vec::new(),
            queue: VecDeque::new(),
            service: VecDeque::new(),
            acks: Vec::new(),
            updates,
            events,
            pfs: None,
            control: Arc::new(Control { connected: AtomicBool::new(true), ..Control::default() }),
            link_up: true,
            last_read: tokio::time::Instant::now(),
            probed: false,
        }
    }

    async fn run(mut self, mut requests: mpsc::UnboundedReceiver<Request>) {
        let mut ping = tokio::time::interval(PING_EVERY);
        let mut tick = tokio::time::interval(TICK);
        let mut ack = tokio::time::interval(ACK_EVERY);
        // Set when the connection is dropped on purpose for a new temp key, which isn't a lost connection.
        let mut renewing = false;
        let mut restarting = false;
        let reason = loop {
            let step = tokio::select! {
                request = requests.recv() => match request {
                    // Queued, not sent, so the exit path fails it with Disconnected where the observer sees it.
                    Some(r) if self.control.closed() => {
                        self.enqueue(r);
                        break Error::Disconnected;
                    }
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
                packet = self.reader.recv() => match packet {
                    Ok(p) => {
                        self.heard();
                        self.handle_packet(p).await
                    }
                    Err(e) => Err(e),
                },
                _ = ping.tick() => self.ping().await,
                _ = tick.tick() => {
                    renewing = self.temp_key_expiring();
                    if renewing { Err(Error::Disconnected) } else { self.tick().await }
                }
                _ = ack.tick() => self.flush().await,
                () = self.control.wake.notified() => {
                    if self.control.closed() {
                        break Error::Disconnected;
                    }
                    restarting = self.control.restart.swap(false, Ordering::AcqRel);
                    if restarting { Err(Error::Disconnected) } else { Ok(()) }
                }
            };
            match step {
                Ok(()) => {}
                Err(e) if self.ends_session(&e) => break e,
                Err(_) => {
                    let (renewing, restarting) = (std::mem::take(&mut renewing), std::mem::take(&mut restarting));
                    self.reconnect(&requests, renewing, restarting).await;
                    ping.reset();
                    // Reconnecting may have used up the close signal.
                    if self.control.closed() {
                        break Error::Disconnected;
                    }
                }
            }
        };
        let code = match reason {
            Error::Transport(code) => Some(code),
            _ => None,
        };
        // With every handle dropped (client gone, or replaced by a DC switch) nobody lost a connection.
        if self.control.closed() || !requests.is_closed() {
            self.link(false);
        }
        self.control.connected.store(false, Ordering::Release);
        let queued = std::mem::take(&mut self.queue).into_iter().map(|p| (0, p));
        for (msg_id, p) in std::mem::take(&mut self.pending).into_iter().chain(queued) {
            self.finish(msg_id, p, Err(code.map_or(Error::Disconnected, Error::Transport)));
        }
    }

    async fn reconnect(&mut self, requests: &mpsc::UnboundedReceiver<Request>, renewing: bool, restarting: bool) {
        let mut delay = RECONNECT_DELAY;
        let mut attempt = 0;
        while !requests.is_closed() && self.updates.as_ref().is_none_or(|u| !u.is_closed()) && !self.control.closed() {
            attempt += 1;
            if !(renewing && attempt == 1) {
                self.link(false);
            }
            // A restart asked for during an outage is this attempt.
            self.control.restart.store(false, Ordering::Release);
            if !skips_backoff(renewing, restarting, attempt) {
                tokio::select! {
                    () = tokio::time::sleep(jitter(delay)) => {}
                    () = self.control.wake.notified() => {}
                }
                delay = (delay * 2).min(MAX_RECONNECT_DELAY);
                if self.control.closed() {
                    return;
                }
            }
            self.drop_abandoned();
            let Ok(mut conn) = Connection::connect(&self.dc, &self.options).await else { continue };
            if self.temp_key_stale() {
                let server = server_key(self.dc);
                let Ok(key) = create_key(&mut conn, self.dc, &server, Some(pfs::TEMP_KEY_LIFETIME)).await else {
                    continue;
                };
                self.use_temp_key(key);
            }
            (self.reader, self.writer) = conn.into_split();
            self.heard();
            if self.resend_pending().await.is_ok() {
                if !(renewing && attempt == 1) {
                    self.events.reconnected();
                    self.events.emit(ClientEvent::Reconnected { dc: self.dc.id, attempt });
                    self.link(true);
                }
                self.request_difference();
                return;
            }
        }
    }

    fn link(&mut self, up: bool) {
        self.control.connected.store(up, Ordering::Release);
        if self.updates.is_some() && up != self.link_up {
            self.link_up = up;
            self.events.link(up);
        }
    }

    fn heard(&mut self) {
        self.last_read = tokio::time::Instant::now();
        self.probed = false;
    }

    async fn tick(&mut self) -> Result<()> {
        // Transfer connections rely on their per-part timeout: a slow part keeps the server silent.
        if self.updates.is_some() {
            // A bind holds back the API requests queued behind it, so they wait on the server as much as sent ones.
            let waiting = !self.pending.is_empty() || (self.unbound() && !self.queue.is_empty());
            let silent = self.last_read.elapsed();
            if silent > DEAD_AFTER || (waiting && silent > PROBE_DEAD_AFTER) {
                return Err(Error::Disconnected);
            }
            if waiting && silent > PROBE_AFTER && !self.probed {
                self.probed = true;
                self.queue_ping();
            }
        }
        self.drop_abandoned();
        if self.service.is_empty() && !self.bind_due() { Ok(()) } else { self.flush().await }
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
        if let Err(mpsc::error::TrySendError::Full(incoming)) = tx.try_send(incoming) {
            self.events.drop_updates(&incoming.updates);
        }
    }

    fn skip(&self, body: &[u8], error: crate::raw::Error) {
        self.events.emit(match error {
            crate::raw::Error::UnexpectedConstructor { id } => ClientEvent::UnknownConstructor { id },
            _ => ClientEvent::UpdateParseFailed { constructor_id: constructor_id(body).unwrap_or(0), len: body.len() },
        });
    }

    // Re-sending a request the server already got runs it twice, so unacked ones get a msgs_state_req first.
    async fn resend_pending(&mut self) -> Result<()> {
        self.checking.clear();
        if self.unbound() {
            // A fresh temp key starts a new session that can't report on the old key's msg_ids, and
            // re-sending could run a request twice: the caller decides.
            for (msg_id, p) in std::mem::take(&mut self.pending) {
                self.finish(msg_id, p, Err(Error::Disconnected));
            }
        } else {
            self.recheck = self.pending.keys().copied().collect();
            self.check_state().await?;
        }
        self.ping().await
    }

    // Bypasses the outbox so we know the msg_id the answer will refer to.
    async fn check_state(&mut self) -> Result<()> {
        let recheck = std::mem::take(&mut self.recheck);
        for chunk in recheck.chunks(MAX_IDS) {
            let ids: Vec<i64> =
                chunk.iter().copied().filter(|id| self.pending.get(id).is_some_and(|p| !p.acked)).collect();
            if ids.is_empty() {
                continue;
            }
            let req = types::MsgsStateReq { msg_ids: ids };
            let (msg_id, seq_no) = (self.msg_ids.next(), self.seq.next(true));
            self.seal(msg_id, seq_no, &req.to_bytes());
            self.writer.finish().await?;
            self.checking.insert(msg_id, req.msg_ids);
        }
        Ok(())
    }

    fn salt(&self) -> i64 {
        self.learned.salt.load(Ordering::Relaxed)
    }

    fn set_salt(&mut self, salt: i64) {
        self.learned.salt.store(salt, Ordering::Relaxed);
    }

    fn set_time_offset(&mut self, time_offset: i64) {
        self.msg_ids.time_offset = time_offset;
        self.learned.time_offset.store(time_offset, Ordering::Relaxed);
    }

    fn enqueue(&mut self, r: Request) {
        let Request { name, body, tag, answer } = r;
        let p = Pending { name, body, tag, answer, start: Instant::now(), container: 0, acked: false, resent: 0 };
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
        self.queue_ping();
        self.flush().await
    }

    fn queue_ping(&mut self) {
        let ping = functions::PingDelayDisconnect { ping_id: i64::from_le_bytes(random()), disconnect_delay: 75 };
        self.service.push_back((ping.to_bytes(), true));
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
            let call = Call {
                name: p.name,
                msg_id,
                request: &p.body,
                duration: p.start.elapsed(),
                result,
                dc: self.dc.id,
                connection: self.connection,
                resent: p.resent,
                tag: p.tag,
            };
            // A panicking observer would otherwise take the connection down with it.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| observe(&call)));
        }
        let _ = p.answer.send(answer);
    }

    fn take_sent(&mut self, msg_id: i64) -> Vec<(i64, Pending)> {
        if self.rebind_if_sent_as(msg_id) {
            return Vec::new();
        }
        if let Some(p) = self.pending.remove(&msg_id) {
            return vec![(msg_id, p)];
        }
        let ids: Vec<i64> = self.pending.iter().filter(|(_, p)| p.container == msg_id).map(|(&id, _)| id).collect();
        ids.into_iter().filter_map(|id| Some((id, self.pending.remove(&id)?))).collect()
    }

    fn resend(&mut self, msg_id: i64) {
        if let Some(ids) = self.checking.remove(&msg_id) {
            self.recheck.extend(ids);
        }
        let sent = self.take_sent(msg_id);
        self.requeue(sent);
    }

    fn requeue(&mut self, sent: Vec<(i64, Pending)>) {
        self.queue.extend(sent.into_iter().map(|(_, p)| Pending { resent: p.resent.saturating_add(1), ..p }));
    }

    async fn handle_packet(&mut self, mut packet: Vec<u8>) -> Result<()> {
        // Drop anything that fails decryption or the MTProto checks: it isn't from the server.
        let Ok((msg_id, seq_no, at)) = mtproto::open(&mut packet, &self.auth_key, &self.auth_key_id, &self.session_id)
        else {
            self.events.emit(ClientEvent::PacketDropped { reason: "failed decryption or the MTProto checks" });
            return Ok(());
        };
        let body = &packet[at];
        if !self.fresh(msg_id, body) {
            return Ok(());
        }
        if constructor_id(body) == Some(msg_container::ID) {
            match msg_container::parse(body) {
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
            self.handle_message(msg_id, seq_no, body);
        }
        if let Some(e) = self.bind_failure() {
            return Err(e);
        }
        self.check_state().await?;
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
        if let Some(Err(error)) = self.options.record.as_ref().map(|r| r.record(false, msg_id, body)) {
            self.events.emit(ClientEvent::RecordingFailed { error: Arc::new(error) });
        }
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
            Some(MSGS_STATE_INFO) => self.handle_state_info(body),
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
                Err(e) => {
                    if let Some(answer) = self.bind_answered(req_msg_id, Err(e.into())) {
                        self.answer(req_msg_id, answer);
                    }
                    return;
                }
            }
        }
        let answer = if constructor_id(&result) == Some(RPC_ERROR) {
            match base::RpcError::from_bytes(&result) {
                Ok(base::RpcError::RpcError(e)) => {
                    Err(Error::Rpc(RpcError::parse(e.error_code, &String::from_utf8_lossy(&e.error_message))))
                }
                Err(e) => Err(e.into()),
            }
        } else {
            self.deliver(&result, true);
            Ok(result)
        };
        if let Some(answer) = self.bind_answered(req_msg_id, answer) {
            self.answer(req_msg_id, answer);
        }
    }
}

/// A key renewal or a restart reconnects at once; every other attempt waits out its backoff first.
fn skips_backoff(renewing: bool, restarting: bool, attempt: u32) -> bool {
    (renewing || restarting) && attempt == 1
}

/// Somewhere in the upper half of `delay`, so clients cut off together don't all come back at once.
fn jitter(delay: Duration) -> Duration {
    let half = delay / 2;
    half + half.mul_f64(f64::from(u32::from_le_bytes(random())) / f64::from(u32::MAX))
}

#[cfg(test)]
mod device_tests {
    use super::init_connection;
    use crate::connection::{Device, Options};
    use crate::raw::{base, functions, types};
    use crate::types::Json;

    #[test]
    fn init_connection_sends_the_device_options() {
        let mut options = Options::default();
        options.device.lang_code = "de".into();
        let sent = init_connection(1, &options, functions::help::GetConfig {}).query;
        assert_eq!((sent.lang_pack.as_str(), sent.system_lang_code.as_str()), ("", "de"));
        assert_eq!(sent.params, None);

        options.device.lang_pack = "android".into();
        options.device.system_lang_code = Some("fr".into());
        options.device.tz_offset = Some(-3600);
        let sent = init_connection(1, &options, functions::help::GetConfig {}).query;
        assert_eq!((sent.lang_pack.as_str(), sent.system_lang_code.as_str()), ("android", "fr"));
        assert_eq!(sent.lang_code, "de");
        let value = base::JSONValue::JsonNumber(types::JsonNumber { value: -3600.0 });
        let entry = types::JsonObjectValue { key: "tz_offset".into(), value };
        let entries = vec![base::JSONObjectValue::JsonObjectValue(Box::new(entry))];
        assert_eq!(sent.params, Some(base::JSONValue::JsonObject(Box::new(types::JsonObject { value: entries }))));
    }

    fn sent_params(set: impl FnOnce(&mut Device)) -> Option<Json> {
        let mut options = Options::default();
        set(&mut options.device);
        init_connection(1, &options, functions::help::GetConfig {}).query.params.map(Json::parse)
    }

    fn object(fields: &[(&str, Json)]) -> Json {
        Json::Object(fields.iter().map(|(key, value)| (key.to_string(), value.clone())).collect())
    }

    #[test]
    fn custom_params_reach_init_connection() {
        let list = Json::Array(vec![Json::Bool(true), Json::Null, Json::String("x".into()), Json::Number(-1.5)]);
        let params = object(&[("nested", object(&[("list", list)])), ("flag", Json::Bool(false))]);
        assert_eq!(sent_params(|d| d.params = Some(params.clone())), Some(params));
    }

    #[test]
    fn tz_offset_replaces_the_params_key() {
        let one = ("a", Json::Number(1.0));
        let old = object(&[one.clone(), ("tz_offset", Json::String("old".into()))]);
        let replaced = sent_params(|d| (d.params, d.tz_offset) = (Some(old), Some(60)));
        assert_eq!(replaced, Some(object(&[one.clone(), ("tz_offset", Json::Number(60.0))])));

        let absent = object(std::slice::from_ref(&one));
        let appended = sent_params(|d| (d.params, d.tz_offset) = (Some(absent), Some(-5)));
        assert_eq!(appended, Some(object(&[one, ("tz_offset", Json::Number(-5.0))])));

        let given = sent_params(|d| (d.params, d.tz_offset) = (Some(Json::Null), Some(1)));
        assert_eq!(given, Some(Json::Null));
    }
}
