use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use tokio::sync::mpsc;

use crate::errors::{Error, Result};
use crate::raw::{LAYER, base, functions};
use crate::session::Session;
use crate::storage::{self, SessionData, session_string};

const DEFAULT_DC: i32 = 2;

pub struct ClientBuilder {
    name: String,
    api_id: i32,
    api_hash: String,
    bot_token: Option<String>,
    session_string: Option<String>,
}

impl ClientBuilder {
    pub fn bot_token(mut self, token: impl Into<String>) -> Self {
        self.bot_token = Some(token.into());
        self
    }

    pub fn session_string(mut self, s: impl Into<String>) -> Self {
        self.session_string = Some(s.into());
        self
    }

    pub async fn start(self) -> Result<Client> {
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
        let (session, updates) = connect(dc_id, auth_key, self.api_id).await?;
        let mut client = Client {
            session,
            updates: tokio::sync::Mutex::new(updates),
            api_id: self.api_id,
            api_hash: self.api_hash,
            storage_path,
            user_id,
            is_bot,
            peers: Mutex::new(HashMap::new()),
        };
        client.save()?;
        if user_id == 0 {
            if let Some(token) = &self.bot_token {
                client.sign_in_bot(token).await?;
            }
        }
        Ok(client)
    }
}

pub struct Client {
    pub(crate) session: Session,
    updates: tokio::sync::Mutex<mpsc::Receiver<base::Updates>>,
    pub(crate) api_id: i32,
    pub(crate) api_hash: String,
    storage_path: Option<PathBuf>,
    pub(crate) user_id: i64,
    pub(crate) is_bot: bool,
    pub(crate) peers: Mutex<HashMap<i64, base::InputPeer>>,
}

impl Client {
    pub fn builder(name: impl Into<String>, api_id: i32, api_hash: impl Into<String>) -> ClientBuilder {
        ClientBuilder { name: name.into(), api_id, api_hash: api_hash.into(), bot_token: None, session_string: None }
    }

    pub async fn next_update(&self) -> Option<base::Updates> {
        self.updates.lock().await.recv().await
    }

    pub(crate) async fn switch_dc(&mut self, dc_id: i32) -> Result<()> {
        let (session, updates) = connect(dc_id, None, self.api_id).await?;
        self.session = session;
        self.updates = tokio::sync::Mutex::new(updates);
        self.user_id = 0;
        self.save()
    }

    pub(crate) fn session_data(&self) -> SessionData {
        SessionData {
            dc_id: self.session.dc_id(),
            api_id: self.api_id,
            auth_key: *self.session.auth_key(),
            user_id: self.user_id,
            is_bot: self.is_bot,
        }
    }

    pub(crate) fn save(&self) -> Result<()> {
        match &self.storage_path {
            Some(path) => Ok(storage::save(path, &self.session_data())?),
            None => Ok(()),
        }
    }
}

async fn connect(
    dc_id: i32,
    auth_key: Option<[u8; 256]>,
    api_id: i32,
) -> Result<(Session, mpsc::Receiver<base::Updates>)> {
    let (session, updates) = Session::connect(dc_id, auth_key).await?;
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
            proxy: None,
            params: None,
            query: functions::help::GetConfig {},
        },
    };
    session.invoke(&init).await?;
    Ok((session, updates))
}
