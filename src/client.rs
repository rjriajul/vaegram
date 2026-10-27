use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use tokio::sync::mpsc;

use crate::connection::{Options, Proxy, Transport};
use crate::errors::{Error, Result};
use crate::handlers::Handler;
use crate::raw::{LAYER, base, functions};
use crate::session::{Incoming, Session};
use crate::storage::{self, SessionData, session_string};

const DEFAULT_DC: i32 = 2;

pub struct ClientBuilder {
    name: String,
    api_id: i32,
    api_hash: String,
    session_string: Option<String>,
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
        let (dc_id, auth_key, user_id, is_bot) = match &stored {
            Some(s) => (s.dc_id, Some(s.auth_key), s.user_id, s.is_bot),
            None => (DEFAULT_DC, None, 0, false),
        };
        let (session, updates) = connect(dc_id, auth_key, self.api_id, &self.options).await?;
        let client = Client(Arc::new(Inner {
            session: RwLock::new(session),
            updates: tokio::sync::Mutex::new(updates),
            api_id: self.api_id,
            api_hash: self.api_hash,
            storage_path,
            options: self.options,
            me: Mutex::new(Me { id: user_id, is_bot, username: None }),
            peers: Mutex::new(HashMap::new()),
            handlers: RwLock::new(Vec::new()),
        }));
        client.save()?;
        Ok((client, self.login))
    }
}

#[derive(Clone)]
pub struct Client(pub(crate) Arc<Inner>);

pub(crate) struct Inner {
    session: RwLock<Session>,
    pub(crate) updates: tokio::sync::Mutex<mpsc::Receiver<Incoming>>,
    pub(crate) api_id: i32,
    pub(crate) api_hash: String,
    storage_path: Option<PathBuf>,
    options: Options,
    pub(crate) me: Mutex<Me>,
    pub(crate) peers: Mutex<HashMap<i64, base::InputPeer>>,
    pub(crate) handlers: RwLock<Vec<Handler>>,
}

pub(crate) struct Me {
    pub(crate) id: i64,
    pub(crate) is_bot: bool,
    pub(crate) username: Option<String>,
}

impl Client {
    pub fn builder(name: impl Into<String>, api_id: i32, api_hash: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            name: name.into(),
            api_id,
            api_hash: api_hash.into(),
            session_string: None,
            login: Login::default(),
            options: Options::default(),
        }
    }

    pub(crate) fn session(&self) -> Session {
        self.0.session.read().unwrap().clone()
    }

    pub(crate) fn set_me(&self, user: &base::User) {
        if let base::User::User(u) = user {
            *self.0.me.lock().unwrap() = Me { id: u.id, is_bot: u.bot, username: u.username.clone() };
        }
    }

    pub(crate) async fn switch_dc(&self, dc_id: i32) -> Result<()> {
        let (session, updates) = connect(dc_id, None, self.0.api_id, &self.0.options).await?;
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
    dc_id: i32,
    auth_key: Option<[u8; 256]>,
    api_id: i32,
    options: &Options,
) -> Result<(Session, mpsc::Receiver<Incoming>)> {
    let (session, updates) = Session::connect(dc_id, auth_key, options).await?;
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
    session.invoke(&init).await?;
    Ok((session, updates))
}
