use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use tokio::sync::Semaphore;

use crate::connection::{Options, Proxy, Transport};
use crate::dispatcher::Routes;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::methods::StreamShared;
use crate::raw::{RemoteCall, base, functions};
use crate::session::internals::data_center::{self, Dc};
use crate::session::transfer::{Lease, TransferPool};
use crate::session::{Events, Session, UpdatesRx, init_connection};
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
    login: Login,
    parse_mode: ParseMode,
    options: Options,
    ordered: bool,
    max_concurrent_handlers: Option<usize>,
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

    /// Runs one chat's handlers one update at a time, in arrival order (by user for inline
    /// queries). Off by default: in a big group, one slow handler then holds up every member.
    pub fn ordered(mut self, ordered: bool) -> Self {
        self.ordered = ordered;
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

    async fn open(self) -> Result<(Client, Login)> {
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
        let first = addr.filter(|a| a.is_ipv6() == self.ipv6).or(built_in).ok_or(Error::InvalidDc(dc_id))?;
        let dc = |addr| Dc { id: dc_id, addr, test: test_mode };
        let state_file = storage_path.as_ref().map(|path| Arc::new(StateFile::new(path.with_extension("state"))));
        let (peers, state) = match &state_file {
            Some(file) => storage::state::load(&file.path, user_id)?,
            None => Default::default(),
        };
        let events = Arc::new(Events::default());
        // A saved address comes from help.getConfig and can go stale.
        let first_try = connect(dc(first), auth_key, self.api_id, &self.options, events.clone()).await;
        let (session, updates, dc_options) = match (first_try, built_in) {
            (Err(_), Some(built_in)) if built_in != first => {
                connect(dc(built_in), auth_key, self.api_id, &self.options, events).await?
            }
            (result, _) => result?,
        };
        let mut inner = Inner::new(session, updates);
        inner.api_id = self.api_id;
        inner.api_hash = self.api_hash;
        inner.dcs = RwLock::new(data_center::table(&dc_options, self.ipv6));
        inner.test_mode = test_mode;
        inner.ipv6 = self.ipv6;
        inner.storage_path = storage_path;
        inner.state_file = state_file;
        inner.options = self.options;
        inner.parse_mode = self.parse_mode;
        inner.state = Mutex::new(state);
        inner.me = Mutex::new(Me { id: user_id, is_bot, username: None });
        inner.peers = Mutex::new(peers);
        inner.ordered = self.ordered;
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
    transfers: TransferPool,
    pub(crate) updates: tokio::sync::Mutex<UpdatesRx>,
    pub(crate) api_id: i32,
    pub(crate) api_hash: String,
    dcs: RwLock<Vec<(i32, SocketAddr)>>,
    test_mode: bool,
    ipv6: bool,
    pub(crate) storage_path: Option<PathBuf>,
    state_file: Option<Arc<StateFile>>,
    options: Options,
    pub(crate) parse_mode: ParseMode,
    pub(crate) state: Mutex<Option<UpdateState>>,
    pub(crate) me: Mutex<Me>,
    pub(crate) peers: Mutex<HashMap<i64, base::InputPeer>>,
    pub(crate) handlers: Routes,
    pub(crate) ordered: bool,
    pub(crate) handler_permits: Option<Arc<Semaphore>>,
    pub(crate) events: Arc<Events>,
    pub(crate) streams: Mutex<HashMap<i64, Arc<StreamShared>>>,
}

pub(crate) use me::Me;

// A public type in a private module, so filters' sealed `Inspect` trait can take it without exporting it.
mod me {
    use std::sync::Arc;

    #[derive(Clone, Debug, Default)]
    pub struct Me {
        pub(crate) id: i64,
        pub(crate) is_bot: bool,
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
            test_mode: false,
            ipv6: false,
            storage_path: None,
            state_file: None,
            options: Options::default(),
            parse_mode: ParseMode::Disabled,
            transfers: TransferPool::default(),
            state: Mutex::new(None),
            me: Mutex::new(Me { id: 0, is_bot: false, username: None }),
            peers: Mutex::new(HashMap::new()),
            handlers: Routes::default(),
            ordered: false,
            handler_permits: None,
            streams: Mutex::new(HashMap::new()),
        }
    }

    fn encode_state(&self, file: &StateFile, state: Option<&UpdateState>) -> (u64, Vec<u8>) {
        let user_id = self.me.lock().unwrap_or_else(PoisonError::into_inner).id;
        let peers = self.peers.lock().unwrap_or_else(PoisonError::into_inner);
        (file.ticket(), storage::state::encode(user_id, &peers, state))
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
            login: Login::default(),
            parse_mode: ParseMode::Disabled,
            options: Options::default(),
            ordered: false,
            max_concurrent_handlers: None,
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
        *client.0.session.write().unwrap() = session;
        (client, sent)
    }

    pub(crate) fn session(&self) -> Session {
        self.0.session.read().unwrap().clone()
    }

    pub(crate) fn set_me(&self, user: &base::User) {
        if let base::User::User(u) = user {
            let username = u.username.as_deref().map(Arc::from);
            *self.0.me.lock().unwrap() = Me { id: u.id, is_bot: u.bot, username };
        }
    }

    pub(crate) fn dc(&self, dc_id: i32) -> Result<Dc> {
        data_center::find(&self.0.dcs.read().unwrap(), dc_id, self.0.test_mode, self.0.ipv6)
            .ok_or(Error::InvalidDc(dc_id))
    }

    pub(crate) async fn switch_dc(&self, dc_id: i32) -> Result<()> {
        let events = self.0.events.clone();
        let (session, updates, dc_options) =
            connect(self.dc(dc_id)?, None, self.0.api_id, &self.0.options, events).await?;
        *self.0.dcs.write().unwrap() = data_center::table(&dc_options, self.0.ipv6);
        *self.0.session.write().unwrap() = session;
        *self.0.updates.lock().await = updates;
        self.0.me.lock().unwrap().id = 0;
        self.0.transfers.clear();
        self.save()
    }

    pub(crate) async fn transfer_sessions(&self, dc_id: i32, count: usize) -> Result<Lease> {
        let dc = self.dc(dc_id)?;
        self.0.transfers.lease(dc, count, &self.session(), self.0.api_id, &self.0.options, &self.0.events).await
    }

    /// Sends a request about an inline message to `dc_id`, the DC its id names, which is the only one that
    /// serves it.
    pub(crate) async fn invoke_inline<R: RemoteCall>(&self, dc_id: i32, request: R) -> Result<R::Return> {
        if dc_id == self.session().dc_id() {
            return self.invoke(&request).await;
        }
        let lease = self.transfer_sessions(dc_id, 1).await?;
        // A reopened file connection hasn't sent its layer, and the answer can carry users.
        lease.sessions[0].invoke(&init_connection(self.0.api_id, &self.0.options, request)).await
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
        match &self.0.storage_path {
            Some(path) => Ok(storage::save(path, &self.session_data())?),
            None => Ok(()),
        }
    }
}

async fn connect(
    dc: Dc,
    auth_key: Option<[u8; 256]>,
    api_id: i32,
    options: &Options,
    events: Arc<Events>,
) -> Result<(Session, UpdatesRx, Vec<base::DcOption>)> {
    let (session, updates) = Session::connect(dc, auth_key, options, events).await?;
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
