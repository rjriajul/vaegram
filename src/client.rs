use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::Duration;

use tokio::sync::Semaphore;

use crate::connection::{Options, Proxy, Transport};
use crate::crypto::rsa::PublicKey;
use crate::dispatcher::Routes;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::methods::{StreamShared, invoke_sleeping_floods};
use crate::raw::{RemoteCall, Serializable, base, functions};
use crate::session::internals::data_center::{self, Dc};
use crate::session::transfer::{Auth, Lease, TransferPool};
use crate::session::{Call, Events, Observer, Salt, Session, UpdatesRx, init_connection};
use crate::storage::recording::Recorder;
use crate::storage::state::{StateFile, UpdateState};
use crate::storage::{self, SessionData, session_string};

const DEFAULT_DC: i32 = 2;

#[must_use = "call .start() or .connect() to get a client"]
pub struct ClientBuilder {
    name: String,
    api_id: i32,
    api_hash: String,
    session_string: Option<String>,
    test_mode: bool,
    ipv6: bool,
    pfs: bool,
    login: Login,
    parse_mode: ParseMode,
    options: Options,
    ordered: bool,
    catch_up: bool,
    max_concurrent_handlers: Option<usize>,
    record: Option<PathBuf>,
}

#[derive(Default)]
pub(crate) struct Login {
    pub(crate) bot_token: Option<String>,
    pub(crate) phone_number: Option<String>,
    pub(crate) phone_code: Option<String>,
    pub(crate) password: Option<String>,
}

impl ClientBuilder {
    pub fn bot_token(mut self, token: impl Into<String>) -> Self {
        self.login.bot_token = Some(token.into());
        self
    }

    pub fn phone_number(mut self, phone_number: impl Into<String>) -> Self {
        self.login.phone_number = Some(phone_number.into());
        self
    }

    pub fn phone_code(mut self, phone_code: impl Into<String>) -> Self {
        self.login.phone_code = Some(phone_code.into());
        self
    }

    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.login.password = Some(password.into());
        self
    }

    pub fn device_model(mut self, model: impl Into<String>) -> Self {
        self.options.device.model = model.into();
        self
    }

    pub fn system_version(mut self, version: impl Into<String>) -> Self {
        self.options.device.system_version = version.into();
        self
    }

    pub fn app_version(mut self, version: impl Into<String>) -> Self {
        self.options.device.app_version = version.into();
        self
    }

    pub fn lang_code(mut self, code: impl Into<String>) -> Self {
        self.options.device.lang_code = code.into();
        self
    }

    pub fn session_string(mut self, s: impl Into<String>) -> Self {
        self.session_string = Some(s.into());
        self
    }

    pub fn transport(mut self, transport: Transport) -> Self {
        self.options.transport = transport;
        self
    }

    /// Disguise MTProto traffic from DPI. MTProxy connections are always obfuscated.
    pub fn obfuscated(mut self) -> Self {
        self.options.obfuscated = true;
        self
    }

    pub fn proxy(mut self, proxy: Proxy) -> Self {
        self.options.proxy = Some(proxy);
        self
    }

    /// Use Telegram's test servers. A stored session keeps the mode it was created with.
    pub fn test_mode(mut self) -> Self {
        self.test_mode = true;
        self
    }

    pub fn ipv6(mut self) -> Self {
        self.ipv6 = true;
        self
    }

    /// Perfect forward secrecy: day-long temporary keys, so a leaked session can't decrypt recorded traffic.
    pub fn pfs(mut self) -> Self {
        self.pfs = true;
        self
    }

    /// Runs one chat's handlers one update at a time, in arrival order (by user for inline
    /// queries). Off by default: in a big group, one slow handler then holds up every member.
    pub fn ordered(mut self, ordered: bool) -> Self {
        self.ordered = ordered;
        self
    }

    /// `false` makes every `run` start from now: what came while the client was stopped is skipped. Gaps while it
    /// runs, such as after a reconnect, are still filled. `true` by default.
    pub fn catch_up(mut self, catch_up: bool) -> Self {
        self.catch_up = catch_up;
        self
    }

    pub fn max_concurrent_handlers(mut self, n: usize) -> Self {
        self.max_concurrent_handlers = Some(n);
        self
    }

    pub fn parse_mode(mut self, mode: ParseMode) -> Self {
        self.parse_mode = mode;
        self
    }

    /// 30 seconds by default; `invoke_with_timeout` overrides it per call.
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.options.request_timeout = timeout;
        self
    }

    /// FLOOD_WAITs up to this long are slept off and retried, at most `max_flood_sleeps` times per call; longer
    /// ones go to the caller as `Error::Rpc`. 60 seconds by default; `Duration::ZERO` hands every one to the caller.
    pub fn flood_sleep_threshold(mut self, threshold: Duration) -> Self {
        self.options.flood_sleep_threshold = threshold;
        self
    }

    /// How many FLOOD_WAITs one call sleeps off, 5 by default; 0 hands every one to the caller. Lost connections
    /// need no retry setting: they are reconnected and the waiting requests sent again on their own.
    pub fn max_flood_sleeps(mut self, n: usize) -> Self {
        self.options.max_flood_sleeps = n;
        self
    }

    /// Update batches waiting for `run`, 1024 by default and at least 1. When it's full, a batch is dropped,
    /// `ClientEvent::UpdatesDropped` fires and a getDifference fetches what it held.
    pub fn updates_capacity(mut self, n: usize) -> Self {
        self.options.updates_capacity = n.max(1);
        self
    }

    /// Sees every request, including the ones vaegram makes on its own (`getDifference`, `initConnection`).
    /// It runs on the connection's task, so it should return quickly.
    pub fn observer(mut self, f: impl Fn(&Call<'_>) + Send + Sync + 'static) -> Self {
        self.options.observer = Some(Observer(Arc::new(f)));
        self
    }

    /// Replaces `path`, which only its owner can read on Unix; [`recording`](crate::storage::recording) says what
    /// goes in, what's left out and how Windows treats the file.
    pub fn record(mut self, path: impl Into<PathBuf>) -> Self {
        self.record = Some(path.into());
        self
    }

    pub async fn start(self) -> Result<Client> {
        let (client, login) = self.open().await?;
        if client.0.me.lock().unwrap().id == 0 {
            client.authorize(login).await?;
        }
        Ok(client)
    }

    pub async fn connect(self) -> Result<Client> {
        Ok(self.open().await?.0)
    }

    async fn open(mut self) -> Result<(Client, Login)> {
        if let Some(path) = &self.record {
            self.options.record = Some(Recorder::create(path)?);
        }
        let (storage_path, stored) = match &self.session_string {
            Some(s) => (None, Some(session_string::parse(s).map_err(Error::InvalidSession)?)),
            None => {
                let path = PathBuf::from(format!("{}.session", self.name));
                let stored = storage::load(&path)?;
                (Some(path), stored)
            }
        };
        let (dc_id, auth_key, user_id, is_bot, test_mode, addr) = match &stored {
            Some(s) => (s.dc_id, Some(s.auth_key), s.user_id, s.is_bot, s.test_mode, s.addr),
            None => (DEFAULT_DC, None, 0, false, self.test_mode, None),
        };
        let built_in = data_center::address(dc_id, test_mode, self.ipv6);
        // A session on localhost (a replay, a test server) is offline by design: every DC is that address, over
        // whichever IP family it has.
        let pinned = addr.filter(|a| a.ip().is_loopback());
        let first = pinned.or_else(|| addr.filter(|a| a.is_ipv6() == self.ipv6)).or(built_in);
        let first = first.ok_or(Error::InvalidDc(dc_id))?;
        let dc = |addr| Dc { id: dc_id, addr, test: test_mode };
        let state_file = storage_path.as_ref().map(|path| Arc::new(StateFile::new(path.with_extension("state"))));
        let (peers, state, salt) = match &state_file {
            Some(file) => storage::state::load(&file.path, user_id)?,
            None => Default::default(),
        };
        let events = Arc::new(Events::default());
        let saved = auth_key.map(|key| (key, salt));
        let try_addr = |addr| connect(dc(addr), saved, self.api_id, &self.options, events.clone(), self.pfs);
        // A saved address comes from help.getConfig and can go stale.
        let (session, updates, dc_options) = match (try_addr(first).await, built_in) {
            (Err(_), Some(built_in)) if built_in != first && pinned.is_none() => try_addr(built_in).await?,
            (result, _) => result?,
        };
        let mut inner = Inner::new(session, updates);
        inner.api_id = self.api_id;
        inner.api_hash = self.api_hash;
        inner.dcs = RwLock::new(data_center::table(&dc_options, self.ipv6, false));
        inner.media_dcs = RwLock::new(data_center::table(&dc_options, self.ipv6, true));
        inner.test_mode = test_mode;
        inner.pinned = pinned;
        inner.ipv6 = self.ipv6;
        inner.storage_path = Mutex::new(storage_path);
        inner.state_file = state_file;
        inner.options = self.options;
        inner.parse_mode = self.parse_mode;
        inner.state = Mutex::new(state);
        inner.me = Mutex::new(Me { id: user_id, is_bot, premium: None, username: None });
        inner.peers = Mutex::new(peers);
        inner.ordered = self.ordered;
        inner.catch_up = self.catch_up;
        inner.handler_permits = self.max_concurrent_handlers.map(|n| Arc::new(Semaphore::new(n.max(1))));
        let client = Client(Arc::new(inner));
        client.save()?;
        Ok((client, self.login))
    }
}

#[derive(Clone)]
pub struct Client(pub(crate) Arc<Inner>);

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client").finish_non_exhaustive()
    }
}

pub(crate) struct Inner {
    session: RwLock<Session>,
    pub(crate) transfers: TransferPool,
    pub(crate) updates: tokio::sync::Mutex<UpdatesRx>,
    pub(crate) api_id: i32,
    pub(crate) api_hash: String,
    dcs: RwLock<Vec<(i32, SocketAddr)>>,
    media_dcs: RwLock<Vec<(i32, SocketAddr)>>,
    cdn_keys: Mutex<Vec<(i32, PublicKey)>>,
    test_mode: bool,
    ipv6: bool,
    pinned: Option<SocketAddr>,
    // Taken by log_out, so a later save can't write the dead login back.
    pub(crate) storage_path: Mutex<Option<PathBuf>>,
    pub(crate) state_file: Option<Arc<StateFile>>,
    pub(crate) options: Options,
    pub(crate) parse_mode: ParseMode,
    pub(crate) state: Mutex<Option<UpdateState>>,
    // Bumped by log_out under the `state` lock, so a dispatcher started for the old account can't hand its
    // state back.
    pub(crate) account: AtomicU64,
    pub(crate) me: Mutex<Me>,
    pub(crate) peers: Mutex<HashMap<i64, base::InputPeer>>,
    pub(crate) handlers: Routes,
    pub(crate) ordered: bool,
    // Whether `run` resumes from the saved update state or starts from now.
    pub(crate) catch_up: bool,
    pub(crate) handler_permits: Option<Arc<Semaphore>>,
    pub(crate) events: Arc<Events>,
    pub(crate) streams: Mutex<HashMap<i64, Arc<StreamShared>>>,
    pub(crate) stop: tokio::sync::watch::Sender<bool>,
}

pub(crate) use me::Me;

// A public type in a private module, so filters' sealed `Inspect` trait can take it without exporting it.
mod me {
    use std::sync::Arc;

    #[derive(Clone, Debug, Default)]
    pub struct Me {
        pub(crate) id: i64,
        pub(crate) is_bot: bool,
        // Looked up again after a reconnect: the subscription can lapse while the client runs.
        pub(crate) premium: Option<bool>,
        pub(crate) username: Option<Arc<str>>,
    }
}

impl Inner {
    fn new(session: Session, updates: UpdatesRx) -> Inner {
        Inner {
            session: RwLock::new(session),
            events: updates.events.clone(),
            updates: tokio::sync::Mutex::new(updates),
            api_id: 0,
            api_hash: String::new(),
            dcs: RwLock::new(Vec::new()),
            media_dcs: RwLock::new(Vec::new()),
            cdn_keys: Mutex::new(Vec::new()),
            test_mode: false,
            ipv6: false,
            pinned: None,
            storage_path: Mutex::new(None),
            state_file: None,
            options: Options::default(),
            parse_mode: ParseMode::Disabled,
            transfers: TransferPool::default(),
            state: Mutex::new(None),
            account: AtomicU64::new(0),
            me: Mutex::new(Me::default()),
            peers: Mutex::new(HashMap::new()),
            handlers: Routes::default(),
            ordered: false,
            catch_up: true,
            handler_permits: None,
            streams: Mutex::new(HashMap::new()),
            stop: tokio::sync::watch::Sender::new(false),
        }
    }

    fn encode_state(&self, file: &StateFile, state: Option<&UpdateState>) -> (u64, Vec<u8>) {
        let user_id = self.me.lock().unwrap_or_else(PoisonError::into_inner).id;
        let salt = self.session.read().unwrap_or_else(PoisonError::into_inner).salt();
        let peers = self.peers.lock().unwrap_or_else(PoisonError::into_inner);
        (file.ticket(), storage::state::encode(user_id, salt, &peers, state))
    }

    /// Returns once the state is on disk, so a stopped client resumes exactly where it left off.
    pub(crate) async fn save_state(&self, state: &UpdateState) {
        let Some(file) = self.state_file.clone() else { return };
        let (ticket, bytes) = self.encode_state(&file, Some(state));
        let _ = tokio::task::spawn_blocking(move || file.write(ticket, &bytes)).await;
    }

    pub(crate) fn save_state_in_background(&self, state: &UpdateState) {
        let Some(file) = self.state_file.clone() else { return };
        let (ticket, bytes) = self.encode_state(&file, Some(state));
        tokio::task::spawn_blocking(move || file.write(ticket, &bytes));
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        let state = self.state.get_mut().ok().and_then(Option::take);
        if let Some(file) = &self.state_file {
            let (ticket, bytes) = self.encode_state(file, state.as_ref());
            file.write(ticket, &bytes);
        }
    }
}

impl Client {
    pub fn builder(name: impl Into<String>, api_id: i32, api_hash: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            name: name.into(),
            api_id,
            api_hash: api_hash.into(),
            session_string: None,
            test_mode: false,
            ipv6: false,
            pfs: false,
            login: Login::default(),
            parse_mode: ParseMode::Disabled,
            options: Options::default(),
            ordered: false,
            catch_up: true,
            max_concurrent_handlers: None,
            record: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn offline() -> Client {
        let mut inner = Inner::new(Session::offline(), UpdatesRx::offline());
        inner.me.get_mut().unwrap().is_bot = true;
        Client(Arc::new(inner))
    }

    #[cfg(test)]
    pub(crate) fn recording() -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        Client::scripted(Vec::new())
    }

    #[cfg(test)]
    pub(crate) fn scripted(replies: Vec<Result<Vec<u8>>>) -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let client = Client::offline();
        let (session, sent) = Session::scripted(replies);
        client.0.transfers.insert(client.dc(2).unwrap(), &session);
        *client.0.session.write().unwrap() = session;
        (client, sent)
    }

    #[cfg(test)]
    pub(crate) fn serve_cdn(&self, dc_id: i32, session: &Session) {
        self.0.dcs.write().unwrap().push((dc_id, SocketAddr::from(([127, 0, 0, 1], 443))));
        self.0.cdn_keys.lock().unwrap().push((dc_id, PublicKey::test()));
        self.0.transfers.insert(self.dc(dc_id).unwrap(), session);
    }

    /// Replaces the options, also on the current session.
    #[cfg(test)]
    pub(crate) fn set_options(&mut self, options: Options) {
        let inner = Arc::get_mut(&mut self.0).expect("set_options needs the only handle");
        inner.session.get_mut().unwrap().set_options(options.clone());
        inner.options = options;
    }

    /// Hands the updates that came inside another reply to the dispatcher, the way the ones on the stream arrive.
    pub(crate) fn feed_updates(&self, updates: base::Updates) {
        self.0.events.nested(updates);
    }

    pub(crate) fn session(&self) -> Session {
        self.0.session.read().unwrap().clone()
    }

    pub(crate) fn set_me(&self, user: &base::User) {
        if let base::User::User(u) = user {
            let username = u.username.as_deref().map(Arc::from);
            *self.0.me.lock().unwrap() = Me { id: u.id, is_bot: u.bot, premium: Some(u.premium), username };
        }
    }

    /// A failed lookup isn't cached; transfers that don't need Premium go on with the regular limits.
    pub(crate) async fn is_premium(&self) -> Result<bool> {
        let reconnected = self.0.events.take_reconnected();
        let known = {
            let mut me = self.0.me.lock().unwrap();
            if reconnected {
                me.premium = None;
            }
            if me.is_bot { Some(false) } else { me.premium }
        };
        if let Some(premium) = known {
            return Ok(premium);
        }
        let me = self.get_me().await?;
        self.set_me(&me);
        Ok(matches!(me, base::User::User(u) if u.premium))
    }

    pub(crate) fn dc(&self, dc_id: i32) -> Result<Dc> {
        if let Some(addr) = self.0.pinned {
            return Ok(Dc { id: dc_id, addr, test: self.0.test_mode });
        }
        data_center::find(&self.0.dcs.read().unwrap(), dc_id, self.0.test_mode, self.0.ipv6)
            .ok_or(Error::InvalidDc(dc_id))
    }

    pub(crate) fn media_dc(&self, dc_id: i32) -> Result<Dc> {
        let media = self.0.media_dcs.read().unwrap().iter().find(|(id, _)| *id == dc_id).map(|&(_, addr)| addr);
        match media.filter(|_| self.0.pinned.is_none()) {
            Some(addr) => Ok(Dc { id: dc_id, addr, test: self.0.test_mode }),
            None => self.dc(dc_id),
        }
    }

    pub(crate) async fn switch_dc(&self, dc_id: i32) -> Result<()> {
        let (inner, events, pfs) = (&self.0, self.0.events.clone(), self.session().pfs());
        let (session, updates, dc_options) =
            connect(self.dc(dc_id)?, None, inner.api_id, &inner.options, events, pfs).await?;
        *self.0.dcs.write().unwrap() = data_center::table(&dc_options, self.0.ipv6, false);
        *self.0.media_dcs.write().unwrap() = data_center::table(&dc_options, self.0.ipv6, true);
        *self.0.session.write().unwrap() = session;
        *self.0.updates.lock().await = updates;
        let mut me = self.0.me.lock().unwrap();
        (me.id, me.premium) = (0, None);
        drop(me);
        self.0.transfers.clear();
        self.save()
    }

    pub(crate) fn forget_account(&self) {
        let mut state = self.0.state.lock().unwrap();
        self.0.account.fetch_add(1, Ordering::Relaxed);
        *state = None;
        drop(state);
        *self.0.me.lock().unwrap() = Me::default();
        self.0.peers.lock().unwrap().clear();
        self.0.transfers.clear();
    }

    pub(crate) async fn transfer_sessions(&self, dc: Dc, count: usize) -> Result<Lease> {
        let home = self.session();
        self.0.transfers.lease(dc, count, Auth::Home(&home, self.0.api_id), &self.0.options, &self.0.events).await
    }

    pub(crate) async fn cdn_sessions(&self, dc: Dc, count: usize) -> Result<Lease> {
        let key = self.cdn_key(dc.id).await?;
        self.0.transfers.lease(dc, count, Auth::Cdn(&key), &self.0.options, &self.0.events).await
    }

    pub(crate) fn forget_cdn(&self, dc: Dc) {
        self.0.transfers.forget(dc);
    }

    async fn cdn_key(&self, dc_id: i32) -> Result<PublicKey> {
        let find = |keys: &[(i32, PublicKey)]| keys.iter().find(|(id, _)| *id == dc_id).map(|(_, key)| key.clone());
        if let Some(key) = find(&self.0.cdn_keys.lock().unwrap()) {
            return Ok(key);
        }
        let base::CdnConfig::CdnConfig(config) = self.invoke(&functions::help::GetCdnConfig {}).await?;
        let keys: Vec<_> = config
            .public_keys
            .iter()
            .filter_map(|base::CdnPublicKey::CdnPublicKey(k)| Some((k.dc_id, PublicKey::from_pem(&k.public_key)?)))
            .collect();
        let key = find(&keys);
        *self.0.cdn_keys.lock().unwrap() = keys;
        key.ok_or(Error::InvalidDc(dc_id))
    }

    /// Sends a request about an inline message to `dc_id`, the DC its id names, which is the only one that
    /// serves it.
    pub(crate) async fn invoke_inline<R: RemoteCall>(&self, dc_id: i32, request: R) -> Result<R::Return> {
        if dc_id == self.session().dc_id() {
            return self.invoke(&request).await;
        }
        let lease = self.transfer_sessions(self.dc(dc_id)?, 1).await?;
        // A reopened file connection hasn't sent its layer, and the answer can carry users.
        let session = &lease.sessions[0];
        let request = init_connection(self.0.api_id, &self.0.options, request);
        invoke_sleeping_floods(session, &request, self.0.options.request_timeout).await
    }

    pub(crate) fn session_data(&self) -> SessionData {
        let session = self.session();
        let me = self.0.me.lock().unwrap();
        SessionData {
            dc_id: session.dc_id(),
            test_mode: self.0.test_mode,
            addr: self.dc(session.dc_id()).ok().map(|dc| dc.addr),
            api_id: self.0.api_id,
            auth_key: *session.auth_key(),
            user_id: me.id,
            is_bot: me.is_bot,
        }
    }

    pub(crate) fn save(&self) -> Result<()> {
        match &*self.0.storage_path.lock().unwrap() {
            Some(path) => Ok(storage::save(path, &self.session_data())?),
            None => Ok(()),
        }
    }
}

async fn connect(
    dc: Dc,
    auth_key: Option<([u8; 256], Salt)>,
    api_id: i32,
    options: &Options,
    events: Arc<Events>,
    pfs: bool,
) -> Result<(Session, UpdatesRx, Vec<base::DcOption>)> {
    let rebind = pfs.then(|| init_connection(api_id, options, functions::help::GetNearestDc {}).to_bytes());
    let (session, updates) = Session::connect(dc, auth_key, options, events, rebind).await?;
    let init = init_connection(api_id, options, functions::help::GetConfig {});
    let base::Config::Config(config) = session.invoke(&init).await?;
    Ok((session, updates, config.dc_options))
}

#[cfg(test)]
pub(crate) async fn sent<F: IntoFuture>(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>, call: F) -> u32 {
    let _ = call.await;
    let body = rx.try_recv().expect("no request sent");
    u32::from_le_bytes(body[..4].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::Client;

    #[test]
    fn builder_defaults_match_todays_behaviour() {
        let builder = Client::builder("defaults", 1, "hash");
        let options = &builder.options;
        assert_eq!((options.max_flood_sleeps, options.updates_capacity), (5, 1024));
        assert_eq!(options.flood_sleep_threshold, Duration::from_secs(60));
        assert!(builder.catch_up);
    }

    #[test]
    fn a_zero_update_queue_is_stored_as_one() {
        assert_eq!(Client::builder("zero", 1, "hash").updates_capacity(0).options.updates_capacity, 1);
    }
}
