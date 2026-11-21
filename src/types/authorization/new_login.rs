use crate::Client;
use crate::raw::types;

/// Someone logged in to this account on a new device, or confirmed such a login.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct NewLogin {
    /// Hash of the new session, the same as [`ActiveSession::id`](crate::types::ActiveSession::id). Pass it to
    /// `terminate_session` to log that device out.
    pub session_id: i64,
    /// `true` while the account owner hasn't confirmed the login. A confirmation comes as another update with this
    /// `false` and the other fields `None`.
    pub is_unconfirmed: bool,
    /// Unix time of the login. `None` once it was confirmed.
    pub date: Option<i32>,
    /// Device the login came from. `None` once it was confirmed.
    pub device: Option<String>,
    /// Where the login came from, as Telegram guessed it. `None` once it was confirmed.
    pub location: Option<String>,
    client: Client,
}

impl NewLogin {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateNewAuthorization) -> Self {
        NewLogin {
            session_id: raw.hash,
            is_unconfirmed: raw.unconfirmed,
            date: raw.date,
            device: raw.device.clone(),
            location: raw.location.clone(),
            client: client.clone(),
        }
    }

    /// The [`Client`] that received this update.
    pub fn client(&self) -> &Client {
        &self.client
    }
}
