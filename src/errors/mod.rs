use std::fmt;
use std::time::Duration;

use crate::raw;
use crate::types::User;

/// The result of every fallible vaegram call; the error is [`Error`] unless given.
pub type Result<T, E = Error> = std::result::Result<T, E>;

const WAITS: [&str; 6] = [
    "FLOOD_WAIT",
    "FLOOD_PREMIUM_WAIT",
    "SLOWMODE_WAIT",
    "FLOOD_TEST_PHONE_WAIT",
    "TAKEOUT_INIT_DELAY",
    "2FA_CONFIRM_WAIT",
];
// A hostile value can't push `Instant` arithmetic past its range.
const MAX_WAIT: Duration = Duration::from_secs(14 * 24 * 60 * 60);

/// An error Telegram answered a request with, such as `400 PEER_ID_INVALID` or `420 FLOOD_WAIT_30`. A trailing
/// number is split off the name into `value`, so match on the name with [`is`](Self::is).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RpcError {
    /// The error code, which works like an HTTP status: 303 for a data center migration, 400 for a bad request,
    /// 401 for a missing or broken login, 420 for flood limits, and so on. See
    /// <https://core.telegram.org/api/errors>.
    pub code: i32,
    /// The error's name, without the trailing number: `FLOOD_WAIT` for `FLOOD_WAIT_30`.
    pub name: String,
    /// The number Telegram put at the end of the name, such as the seconds of a FLOOD_WAIT or the data center of a
    /// `*_MIGRATE`; `None` when the name doesn't end in one. Numbers past `u32::MAX` are kept as `u32::MAX`.
    pub value: Option<u32>,
}

impl RpcError {
    pub(crate) fn new(code: i32, message: &str) -> Self {
        match message.rsplit_once('_') {
            // All digits, so parsing fails only past u32::MAX; such a wait still counts, at the cap.
            Some((name, n)) if !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()) => {
                Self { code, name: name.into(), value: Some(n.parse().unwrap_or(u32::MAX)) }
            }
            _ => Self { code, name: message.into(), value: None },
        }
    }

    /// Compares the name without its value: FLOOD_WAIT_30 is `"FLOOD_WAIT"`.
    pub fn is(&self, name: &str) -> bool {
        self.name == name
    }

    /// How long to wait before the same request can work, for the 420 errors that mean that, at most 14 days
    /// (`FLOOD_SKIP_FAILED_WAIT` is one second). Lookalikes such as `PREMIUM_SUB_ACTIVE_UNTIL_<date>` or
    /// `STORY_SEND_FLOOD_<n>` give `None`.
    pub fn flood_wait(&self) -> Option<Duration> {
        if self.code != 420 {
            return None;
        }
        if self.name == "FLOOD_SKIP_FAILED_WAIT" {
            return Some(Duration::from_secs(1));
        }
        let wait = WAITS.contains(&self.name.as_str()).then_some(self.value?)?;
        Some(Duration::from_secs(wait.into()).min(MAX_WAIT))
    }

    /// The data center in any `*_MIGRATE_<dc>` error: `USER_`, `PHONE_` and `NETWORK_MIGRATE` move the account,
    /// while `FILE_MIGRATE` and `STATS_MIGRATE` only say where that one request goes.
    pub fn migrate_dc(&self) -> Option<i32> {
        self.value.filter(|_| self.name.ends_with("_MIGRATE")).and_then(|dc| dc.try_into().ok())
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

/// Everything a vaegram call can fail with. Telegram's own errors are [`Error::Rpc`]; [`is`](Self::is) and
/// [`flood_wait`](Self::flood_wait) look into them without a match.
#[derive(Debug)]
#[non_exhaustive]
pub enum Error {
    /// Telegram answered the request with an error.
    Rpc(RpcError),
    /// A network or file error: the connection failed, a socket timed out, a file couldn't be read or written.
    Io(std::io::Error),
    /// The server refused the connection with this transport error code, made positive. 404 usually means the data
    /// center doesn't know the auth key; on the main connection that's the stored login. It's also sent for a
    /// malformed packet. See <https://core.telegram.org/mtproto/mtproto-transports#transport-errors>.
    Transport(i32),
    /// Telegram's answer couldn't be read as the TL type expected.
    Deserialize(raw::Error),
    /// A security check failed, such as a nonce, hash or DH value in the key exchange, so the data was refused.
    Security(&'static str),
    /// Telegram refused the message carrying the request with this `bad_msg_notification` code. The time and
    /// sequence codes (16, 17, 32, 33) are fixed and resent instead, so they never get here. See
    /// <https://core.telegram.org/mtproto/service_messages_about_messages>.
    BadMessage(i32),
    /// The connection the call needed was closed before an answer came: by `disconnect` or `stop_transmission`,
    /// because its session ended or every handle to it was dropped, or `run` stopped while a `recover_gaps` waited.
    Disconnected,
    /// The request got no answer within its timeout and was abandoned. `DnsConfig::fetch` (`dns-config` feature)
    /// also returns it when a DNS resolver doesn't answer in time.
    Timeout,
    /// No address is known for this data center id.
    InvalidDc(i32),
    /// The chat or user with this id (Bot API style) can't be used: it's neither in storage nor found by asking
    /// Telegram, or the call needs a user and the id names a group or channel.
    PeerNotFound(i64),
    /// An invite link to a private chat this account hasn't joined; join it first.
    ChatNotJoined,
    /// Joining needs the approval of the chat's guard bot: open its web app with `query_id`, then retry.
    GuardBotApprovalRequired {
        /// The guard bot whose web app approves the join.
        bot: Box<User>,
        /// The web app query the approval belongs to.
        query_id: i64,
    },
    /// An answer vaegram can't use, such as no chat in the answer to a join; the text says what.
    Unexpected(&'static str),
    /// An argument or setting that can't work; the text says which and why.
    InvalidArgument(&'static str),
    /// The session string passed to `ClientBuilder::session_string` can't be read; the text says why.
    InvalidSession(&'static str),
    /// The phone number has no Telegram account yet: create one with `sign_up`.
    SignUpRequired,
    /// A file can't be uploaded: it's empty (`size` 0) or bigger than this account may send.
    FileSize {
        /// The file's size in bytes.
        size: u64,
        /// The largest file this account may upload, in bytes; it's higher for Premium accounts.
        limit: u64,
    },
    /// No matching update arrived before a listener's timeout.
    ListenerTimeout,
    /// The listener was stopped by `stop_listening`, or because `run` stopped; new listeners fail the same way
    /// until the next `run`.
    /// A listener started before the first `run` isn't stopped: it waits until `ListenerTimeout`.
    ListenerStopped,
    /// `max_listeners` listeners are already waiting, so this one wasn't started.
    ListenerLimitReached(usize),
}

impl Error {
    /// The Telegram error inside, or `None` for any other error.
    pub fn rpc(&self) -> Option<&RpcError> {
        if let Error::Rpc(e) = self { Some(e) } else { None }
    }

    /// Whether this is a Telegram error with this name, compared without its value: `"FLOOD_WAIT"` matches
    /// `FLOOD_WAIT_30`. Always `false` for any other error.
    pub fn is(&self, name: &str) -> bool {
        self.rpc().is_some_and(|e| e.is(name))
    }

    /// [`RpcError::flood_wait`] of a Telegram error; `None` for any other error.
    pub fn flood_wait(&self) -> Option<Duration> {
        self.rpc().and_then(RpcError::flood_wait)
    }
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
            Error::GuardBotApprovalRequired { .. } => {
                f.write_str("joining this chat needs the approval of its guard bot's web app")
            }
            Error::Unexpected(what) => write!(f, "unexpected answer from Telegram: {what}"),
            Error::InvalidArgument(what) => write!(f, "invalid argument: {what}"),
            Error::InvalidSession(why) => write!(f, "invalid session string: {why}"),
            Error::SignUpRequired => {
                f.write_str("this phone number has no Telegram account yet; create one with sign_up")
            }
            Error::FileSize { size: 0, .. } => f.write_str("can't upload an empty file"),
            Error::FileSize { size, limit } => {
                write!(f, "can't upload {size} bytes: this account can send files of up to {} MiB", limit >> 20)
            }
            Error::ListenerTimeout => f.write_str("no matching update arrived before the timeout"),
            Error::ListenerStopped => f.write_str("the listener was stopped"),
            Error::ListenerLimitReached(max) => write!(f, "{max} listeners are already waiting"),
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

impl From<std::convert::Infallible> for Error {
    fn from(e: std::convert::Infallible) -> Self {
        match e {}
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Error, RpcError};

    #[test]
    fn splits_the_number_off() {
        let e = RpcError::new(420, "FLOOD_WAIT_30");
        assert_eq!((e.name.as_str(), e.value), ("FLOOD_WAIT", Some(30)));

        let e = RpcError::new(400, "PEER_ID_INVALID");
        assert_eq!((e.name.as_str(), e.value), ("PEER_ID_INVALID", None));

        let e = RpcError::new(303, "USER_MIGRATE_4");
        assert_eq!((e.name.as_str(), e.value), ("USER_MIGRATE", Some(4)));

        let e = RpcError::new(420, "2FA_CONFIRM_WAIT_604800");
        assert_eq!((e.name.as_str(), e.value), ("2FA_CONFIRM_WAIT", Some(604800)));

        let e = RpcError::new(400, "FILE_PART_5_MISSING");
        assert_eq!((e.name.as_str(), e.value), ("FILE_PART_5_MISSING", None));
    }

    #[test]
    fn names_match_without_the_value() {
        let e = RpcError::new(420, "FLOOD_WAIT_30");
        assert!(e.is("FLOOD_WAIT") && !e.is("FLOOD_WAIT_30") && !e.is("FLOOD") && !e.is("FLOOD_PREMIUM_WAIT"));
        assert!(RpcError::new(420, "2FA_CONFIRM_WAIT_604800").is("2FA_CONFIRM_WAIT"));

        let e = Error::Rpc(RpcError::new(400, "MESSAGE_NOT_MODIFIED"));
        assert!(e.is("MESSAGE_NOT_MODIFIED") && !e.is("PEER_ID_INVALID"));
        assert!(!Error::Disconnected.is("MESSAGE_NOT_MODIFIED") && Error::Disconnected.rpc().is_none());
    }

    #[test]
    fn waits_and_migrations_carry_their_value() {
        let wait = |code, msg| RpcError::new(code, msg).flood_wait();
        assert_eq!(wait(420, "FLOOD_WAIT_30"), Some(Duration::from_secs(30)));
        assert_eq!(wait(420, "SLOWMODE_WAIT_10"), Some(Duration::from_secs(10)));
        assert_eq!(wait(420, "FLOOD_WAIT"), None);
        assert_eq!(wait(420, "FLOOD_PREMIUM_WAIT_5"), Some(Duration::from_secs(5)));
        assert_eq!(wait(420, "FLOOD_TEST_PHONE_WAIT_3"), Some(Duration::from_secs(3)));
        assert_eq!(wait(420, "TAKEOUT_INIT_DELAY_3600"), Some(Duration::from_secs(3600)));
        assert_eq!(wait(420, "2FA_CONFIRM_WAIT_604800"), Some(Duration::from_secs(604800)));
        assert_eq!(wait(420, "FLOOD_SKIP_FAILED_WAIT"), Some(Duration::from_secs(1)));
        assert_eq!(wait(420, "FLOOD_WAIT_4294967295"), Some(Duration::from_secs(14 * 24 * 3600)));
        assert_eq!(wait(420, "FLOOD_WAIT_4294967296"), Some(Duration::from_secs(14 * 24 * 3600)));
        assert_eq!(wait(420, "FLOOD_WAIT_99999999999999999999"), Some(Duration::from_secs(14 * 24 * 3600)));
        for lookalike in ["PREMIUM_SUB_ACTIVE_UNTIL_1700000000", "STORY_SEND_FLOOD_60", "FROZEN_METHOD_INVALID"] {
            assert_eq!(wait(420, lookalike), None, "{lookalike}");
        }
        assert_eq!(wait(420, "ADDRESS_INVALID"), None);
        assert_eq!(wait(303, "USER_MIGRATE_4"), None);
        assert_eq!(Error::Rpc(RpcError::new(420, "FLOOD_WAIT_5")).flood_wait(), Some(Duration::from_secs(5)));

        let dc = |msg| RpcError::new(303, msg).migrate_dc();
        assert_eq!((dc("USER_MIGRATE_4"), dc("PHONE_MIGRATE_2"), dc("FILE_MIGRATE_5")), (Some(4), Some(2), Some(5)));
        assert_eq!((dc("NETWORK_MIGRATE"), RpcError::new(420, "FLOOD_WAIT_4").migrate_dc()), (None, None));
    }

    #[test]
    fn display_keeps_the_telegram_text() {
        assert_eq!(RpcError::new(420, "FLOOD_WAIT_30").to_string(), "Telegram error 420: FLOOD_WAIT_30");
        let e = Error::Rpc(RpcError::new(400, "PEER_ID_INVALID"));
        assert_eq!(e.to_string(), "Telegram error 400: PEER_ID_INVALID");
    }
}
