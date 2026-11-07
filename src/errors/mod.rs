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
#[non_exhaustive]
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
    /// An error with this code, name and number, as Telegram splits `FLOOD_WAIT_30` into `"FLOOD_WAIT"` and
    /// `Some(30)`; for building errors in tests or a custom retry policy. `name` excludes the trailing number, which
    /// goes in `value`: `new(420, "FLOOD_WAIT_30", None)` prints the same but [`is`](Self::is)`("FLOOD_WAIT")` and
    /// [`flood_wait`](Self::flood_wait) don't see it.
    pub fn new(code: i32, name: impl Into<String>, value: Option<u32>) -> Self {
        RpcError { code, name: name.into(), value }
    }

    /// Splits a trailing number off Telegram's error message: `FLOOD_WAIT_30` is `FLOOD_WAIT` with 30.
    pub(crate) fn parse(code: i32, message: &str) -> Self {
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

/// Everything a vaegram call can fail with. Telegram's own errors are [`Error::Rpc`]; [`is`](Self::is),
/// [`flood_wait`](Self::flood_wait) and the `is_*` helpers look into them without a match.
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
    /// A security check failed, such as a nonce, hash or DH value in the key exchange, or the hash of Mini App data
    /// in `utils::validate_web_app_data`, so the data was refused.
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
    #[non_exhaustive]
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
    #[non_exhaustive]
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

// `is_<name>` helpers on `Error`, each an exact [`Error::is`] check of one Telegram error name.
macro_rules! named_errors {
    ($($(#[$doc:meta])* $fn:ident => $name:literal),* $(,)?) => {$(
        $(#[$doc])* ///
        /// Always `false` for any other error.
        pub fn $fn(&self) -> bool {
            self.is($name)
        }
    )*};
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

    /// Whether this is a 420 error that asks to wait before retrying, the ones [`flood_wait`](Self::flood_wait)
    /// gives a duration for: `FLOOD_WAIT_X`, `FLOOD_PREMIUM_WAIT_X`, `SLOWMODE_WAIT_X` and the other waits. Read
    /// the duration with `flood_wait`.
    pub fn is_flood_wait(&self) -> bool {
        self.flood_wait().is_some()
    }

    /// Whether this is a 401 Telegram error: the login is missing, revoked or deactivated (`AUTH_KEY_UNREGISTERED`,
    /// `SESSION_REVOKED`, `USER_DEACTIVATED`, …), or, while logging in, it still needs the two-step verification
    /// password (`SESSION_PASSWORD_NEEDED`).
    pub fn is_unauthorized(&self) -> bool {
        self.rpc().is_some_and(|e| e.code == 401)
    }

    /// Whether this is a `FILE_REFERENCE_*` error, such as `FILE_REFERENCE_EXPIRED`, `FILE_REFERENCE_INVALID` or
    /// `FILE_REFERENCE_<n>_EXPIRED` for one item of an album: the file reference is stale, so fetch the message or
    /// other source of the file again for a fresh one. Downloads of a known source already do that once.
    pub fn is_file_reference_error(&self) -> bool {
        self.rpc().is_some_and(|e| e.name.starts_with("FILE_REFERENCE_"))
    }

    named_errors! {
        /// Whether this is `MESSAGE_NOT_MODIFIED`: an edit that changes nothing.
        is_message_not_modified => "MESSAGE_NOT_MODIFIED",
        /// Whether this is `MESSAGE_ID_INVALID`: the message doesn't exist, or this account can't act on it.
        is_message_id_invalid => "MESSAGE_ID_INVALID",
        /// Whether this is `PEER_ID_INVALID`: the chat or user can't be used with this account.
        is_peer_id_invalid => "PEER_ID_INVALID",
        /// Whether this is `USER_IS_BLOCKED`: the user blocked this account, or stopped this bot.
        is_user_is_blocked => "USER_IS_BLOCKED",
        /// Whether this is `CHAT_ADMIN_REQUIRED`: the call needs admin rights this account doesn't have.
        is_chat_admin_required => "CHAT_ADMIN_REQUIRED",
        /// Whether this is `CHAT_WRITE_FORBIDDEN`: this account can't send messages in the chat.
        is_chat_write_forbidden => "CHAT_WRITE_FORBIDDEN",
        /// Whether this is `USER_NOT_PARTICIPANT`: the user isn't a member of the chat.
        is_user_not_participant => "USER_NOT_PARTICIPANT",
        /// Whether this is `CHANNEL_PRIVATE`: the channel or supergroup can't be seen, for example after a ban.
        is_channel_private => "CHANNEL_PRIVATE",
        /// Whether this is `QUERY_ID_INVALID`: the callback or inline query was already answered or has expired.
        is_query_id_invalid => "QUERY_ID_INVALID",
        /// Whether this is `AUTH_KEY_UNREGISTERED`: the stored login isn't valid any more, so log in again.
        is_auth_key_unregistered => "AUTH_KEY_UNREGISTERED",
        /// Whether this is `SESSION_PASSWORD_NEEDED`: the account has two-step verification, so finish the login
        /// with its password.
        is_session_password_needed => "SESSION_PASSWORD_NEEDED",
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
        let e = RpcError::parse(420, "FLOOD_WAIT_30");
        assert_eq!((e.name.as_str(), e.value), ("FLOOD_WAIT", Some(30)));

        let e = RpcError::parse(400, "PEER_ID_INVALID");
        assert_eq!((e.name.as_str(), e.value), ("PEER_ID_INVALID", None));

        let e = RpcError::parse(303, "USER_MIGRATE_4");
        assert_eq!((e.name.as_str(), e.value), ("USER_MIGRATE", Some(4)));

        let e = RpcError::parse(420, "2FA_CONFIRM_WAIT_604800");
        assert_eq!((e.name.as_str(), e.value), ("2FA_CONFIRM_WAIT", Some(604800)));

        let e = RpcError::parse(400, "FILE_PART_5_MISSING");
        assert_eq!((e.name.as_str(), e.value), ("FILE_PART_5_MISSING", None));

        assert_eq!(RpcError::new(420, "FLOOD_WAIT", Some(30)), RpcError::parse(420, "FLOOD_WAIT_30"));
    }

    #[test]
    fn names_match_without_the_value() {
        let e = RpcError::parse(420, "FLOOD_WAIT_30");
        assert!(e.is("FLOOD_WAIT") && !e.is("FLOOD_WAIT_30") && !e.is("FLOOD") && !e.is("FLOOD_PREMIUM_WAIT"));
        assert!(RpcError::parse(420, "2FA_CONFIRM_WAIT_604800").is("2FA_CONFIRM_WAIT"));

        let e = Error::Rpc(RpcError::parse(400, "MESSAGE_NOT_MODIFIED"));
        assert!(e.is("MESSAGE_NOT_MODIFIED") && !e.is("PEER_ID_INVALID"));
        assert!(!Error::Disconnected.is("MESSAGE_NOT_MODIFIED") && Error::Disconnected.rpc().is_none());
    }

    #[test]
    fn named_helpers_match_their_errors_only() {
        type Case = (fn(&Error) -> bool, i32, &'static str);
        let rpc = |code: i32, msg: &str| Error::Rpc(RpcError::parse(code, msg));
        let helpers: [Case; 11] = [
            (Error::is_message_not_modified, 400, "MESSAGE_NOT_MODIFIED"),
            (Error::is_message_id_invalid, 400, "MESSAGE_ID_INVALID"),
            (Error::is_peer_id_invalid, 400, "PEER_ID_INVALID"),
            (Error::is_user_is_blocked, 400, "USER_IS_BLOCKED"),
            (Error::is_chat_admin_required, 400, "CHAT_ADMIN_REQUIRED"),
            (Error::is_chat_write_forbidden, 403, "CHAT_WRITE_FORBIDDEN"),
            (Error::is_user_not_participant, 400, "USER_NOT_PARTICIPANT"),
            (Error::is_channel_private, 406, "CHANNEL_PRIVATE"),
            (Error::is_query_id_invalid, 400, "QUERY_ID_INVALID"),
            (Error::is_auth_key_unregistered, 401, "AUTH_KEY_UNREGISTERED"),
            (Error::is_session_password_needed, 401, "SESSION_PASSWORD_NEEDED"),
        ];
        for (i, (helper, code, name)) in helpers.iter().enumerate() {
            assert!(helper(&rpc(*code, name)), "{name}");
            for (j, (_, other_code, other)) in helpers.iter().enumerate() {
                assert_eq!(helper(&rpc(*other_code, other)), i == j, "{name} on {other}");
            }
            assert!(!helper(&Error::Disconnected) && !helper(&Error::Unexpected("PEER_ID_INVALID")), "{name}");
            assert!(!helper(&rpc(*code, &format!("{name}_X"))), "{name}");
        }

        assert!(rpc(420, "FLOOD_WAIT_30").is_flood_wait() && rpc(420, "FLOOD_PREMIUM_WAIT_5").is_flood_wait());
        assert!(rpc(420, "SLOWMODE_WAIT_10").is_flood_wait() && rpc(420, "FLOOD_SKIP_FAILED_WAIT").is_flood_wait());
        assert!(!rpc(420, "FLOOD_WAIT").is_flood_wait() && !rpc(420, "STORY_SEND_FLOOD_60").is_flood_wait());
        assert!(!rpc(400, "FLOOD_WAIT_30").is_flood_wait() && !Error::Timeout.is_flood_wait());

        assert!(rpc(401, "AUTH_KEY_UNREGISTERED").is_unauthorized() && rpc(401, "SESSION_REVOKED").is_unauthorized());
        assert!(!rpc(400, "PEER_ID_INVALID").is_unauthorized() && !Error::Transport(404).is_unauthorized());

        for name in ["FILE_REFERENCE_EXPIRED", "FILE_REFERENCE_INVALID", "FILE_REFERENCE_3_EXPIRED"] {
            assert!(rpc(400, name).is_file_reference_error(), "{name}");
        }
        assert!(
            !rpc(400, "FILE_REFERENCE").is_file_reference_error()
                && !rpc(400, "FILE_ID_INVALID").is_file_reference_error()
        );
        assert!(!Error::Disconnected.is_file_reference_error());
    }

    #[test]
    fn waits_and_migrations_carry_their_value() {
        let wait = |code, msg| RpcError::parse(code, msg).flood_wait();
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
        assert_eq!(Error::Rpc(RpcError::parse(420, "FLOOD_WAIT_5")).flood_wait(), Some(Duration::from_secs(5)));

        let dc = |msg| RpcError::parse(303, msg).migrate_dc();
        assert_eq!((dc("USER_MIGRATE_4"), dc("PHONE_MIGRATE_2"), dc("FILE_MIGRATE_5")), (Some(4), Some(2), Some(5)));
        assert_eq!((dc("NETWORK_MIGRATE"), RpcError::parse(420, "FLOOD_WAIT_4").migrate_dc()), (None, None));
    }

    #[test]
    fn display_keeps_the_telegram_text() {
        assert_eq!(RpcError::parse(420, "FLOOD_WAIT_30").to_string(), "Telegram error 420: FLOOD_WAIT_30");
        let e = Error::Rpc(RpcError::parse(400, "PEER_ID_INVALID"));
        assert_eq!(e.to_string(), "Telegram error 400: PEER_ID_INVALID");
    }
}
