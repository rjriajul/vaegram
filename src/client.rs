use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError, RwLock};

use crate::connection::{Options, Proxy, Transport};
use crate::errors::{Error, Result};
use crate::handlers::Handler;
use crate::raw::{LAYER, base, functions};
use crate::session::internals::data_center::{self, Dc};
use crate::session::{Session, UpdatesRx};
use crate::storage::state::{StateFile, UpdateState};
use crate::storage::{self, SessionData, session_string};

const DEFAULT_DC: i32 = 2;

pub struct ClientBuilder {
    name: String,
    api_id: i32,
    api_hash: String,
    session_string: Option<String>,
    test_mode: bool,
    ipv6: bool,
    login: Login,
    options: Options,
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
        // A saved address comes from help.getConfig and can go stale.
        let first_try = connect(dc(first), auth_key, self.api_id, &self.options).await;
        let (session, updates, dc_options) = match (first_try, built_in) {
            (Err(_), Some(built_in)) if built_in != first => {
                connect(dc(built_in), auth_key, self.api_id, &self.options).await?
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
        inner.state = Mutex::new(state);
        inner.me = Mutex::new(Me { id: user_id, is_bot, username: None });
        inner.peers = Mutex::new(peers);
        let client = Client(Arc::new(inner));
        client.save()?;
        Ok((client, self.login))
    }
}

#[derive(Clone)]
pub struct Client(pub(crate) Arc<Inner>);

pub(crate) struct Inner {
    session: RwLock<Session>,
    pub(crate) updates: tokio::sync::Mutex<UpdatesRx>,
    pub(crate) api_id: i32,
    pub(crate) api_hash: String,
    dcs: RwLock<Vec<(i32, SocketAddr)>>,
    test_mode: bool,
    ipv6: bool,
    storage_path: Option<PathBuf>,
    state_file: Option<Arc<StateFile>>,
    options: Options,
    pub(crate) state: Mutex<Option<UpdateState>>,
    pub(crate) me: Mutex<Me>,
    pub(crate) peers: Mutex<HashMap<i64, base::InputPeer>>,
    pub(crate) handlers: RwLock<Vec<Handler>>,
}

pub(crate) struct Me {
    pub(crate) id: i64,
    pub(crate) is_bot: bool,
    pub(crate) username: Option<String>,
}

impl Inner {
    fn new(session: Session, updates: UpdatesRx) -> Inner {
        Inner {
            session: RwLock::new(session),
            updates: tokio::sync::Mutex::new(updates),
            api_id: 0,
            api_hash: String::new(),
            dcs: RwLock::new(Vec::new()),
            test_mode: false,
            ipv6: false,
            storage_path: None,
            state_file: None,
            options: Options::default(),
            state: Mutex::new(None),
            me: Mutex::new(Me { id: 0, is_bot: false, username: None }),
            peers: Mutex::new(HashMap::new()),
            handlers: RwLock::new(Vec::new()),
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
            options: Options::default(),
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
            *self.0.me.lock().unwrap() = Me { id: u.id, is_bot: u.bot, username: u.username.clone() };
        }
    }

    pub(crate) fn dc(&self, dc_id: i32) -> Result<Dc> {
        data_center::find(&self.0.dcs.read().unwrap(), dc_id, self.0.test_mode, self.0.ipv6)
            .ok_or(Error::InvalidDc(dc_id))
    }

    pub(crate) async fn switch_dc(&self, dc_id: i32) -> Result<()> {
        let (session, updates, dc_options) = connect(self.dc(dc_id)?, None, self.0.api_id, &self.0.options).await?;
        *self.0.dcs.write().unwrap() = data_center::table(&dc_options, self.0.ipv6);
        *self.0.session.write().unwrap() = session;
        *self.0.updates.lock().await = updates;
        self.0.me.lock().unwrap().id = 0;
        self.save()
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
) -> Result<(Session, UpdatesRx, Vec<base::DcOption>)> {
    let (session, updates) = Session::connect(dc, auth_key, options).await?;
    let init = functions::InvokeWithLayer {
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
            query: functions::help::GetConfig {},
        },
    };
    let base::Config::Config(config) = session.invoke(&init).await?;
    Ok((session, updates, config.dc_options))
}

#[cfg(test)]
pub(crate) async fn sent<F: IntoFuture>(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>, call: F) -> u32 {
    let _ = call.await;
    let body = rx.try_recv().expect("no request sent");
    u32::from_le_bytes(body[..4].try_into().unwrap())
}
