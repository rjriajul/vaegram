use std::fmt;

use crate::raw;

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcError {
    pub code: i32,
    pub name: String,
    pub value: Option<u32>,
}

impl RpcError {
    pub(crate) fn new(code: i32, message: &str) -> Self {
        match message.rsplit_once('_') {
            Some((name, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
                Self { code, name: name.into(), value: n.parse().ok() }
            }
            _ => Self { code, name: message.into(), value: None },
        }
    }
}

impl fmt::Display for RpcError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.value {
            Some(v) => write!(f, "Telegram error {}: {}_{v}", self.code, self.name),
            None => write!(f, "Telegram error {}: {}", self.code, self.name),
        }
    }
}

impl std::error::Error for RpcError {}

#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    Rpc(RpcError),
    Io(std::io::Error),
    Transport(i32),
    Deserialize(raw::Error),
    Security(&'static str),
    BadMessage(i32),
    Disconnected,
    Timeout,
    InvalidDc(i32),
    PeerNotFound(i64),
    /// An invite link to a private chat this account hasn't joined; join it first.
    ChatNotJoined,
    Unexpected(&'static str),
    InvalidArgument(&'static str),
    InvalidSession(&'static str),
    SignUpRequired,
    FileSize {
        size: u64,
        limit: u64,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Rpc(e) => e.fmt(f),
            Error::Io(e) => write!(f, "network error: {e}"),
            Error::Transport(404) => f.write_str("transport error 404: auth key not found; delete the session file"),
            Error::Transport(code) => write!(f, "transport error {code}"),
            Error::Deserialize(e) => write!(f, "bad data from Telegram: {e}"),
            Error::Security(what) => write!(f, "security check failed: {what}"),
            Error::BadMessage(code) => write!(f, "Telegram rejected the message (bad_msg_notification {code})"),
            Error::Disconnected => f.write_str("disconnected from Telegram"),
            Error::Timeout => f.write_str("Telegram did not answer in time"),
            Error::InvalidDc(dc) => write!(f, "unknown data center {dc}"),
            Error::PeerNotFound(id) => write!(f, "chat {id} not found for this account"),
            Error::ChatNotJoined => f.write_str("this account hasn't joined the invite link's chat"),
            Error::Unexpected(what) => write!(f, "unexpected answer from Telegram: {what}"),
            Error::InvalidArgument(what) => write!(f, "invalid argument: {what}"),
            Error::InvalidSession(why) => write!(f, "invalid session string: {why}"),
            Error::SignUpRequired => {
                f.write_str("this phone number has no Telegram account yet; sign up in an app first")
            }
            Error::FileSize { size: 0, .. } => f.write_str("can't upload an empty file"),
            Error::FileSize { size, limit } => {
                write!(f, "can't upload {size} bytes: this account can send files of up to {} MiB", limit >> 20)
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Rpc(e) => Some(e),
            Error::Io(e) => Some(e),
            Error::Deserialize(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

impl From<raw::Error> for Error {
    fn from(e: raw::Error) -> Self {
        Error::Deserialize(e)
    }
}

#[cfg(test)]
mod tests {
    use super::RpcError;

    #[test]
    fn splits_the_number_off() {
        let e = RpcError::new(420, "FLOOD_WAIT_30");
        assert_eq!((e.name.as_str(), e.value), ("FLOOD_WAIT", Some(30)));

        let e = RpcError::new(400, "PEER_ID_INVALID");
        assert_eq!((e.name.as_str(), e.value), ("PEER_ID_INVALID", None));

        let e = RpcError::new(303, "USER_MIGRATE_4");
        assert_eq!((e.name.as_str(), e.value), ("USER_MIGRATE", Some(4)));
    }
}
