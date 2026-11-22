use std::fmt;

use crate::raw::{base, types};

/// Channel ids in the Bot-API form are `-1000000000000 - channel_id`.
const CHANNEL_OFFSET: i64 = 1_000_000_000_000;

/// Which kind of peer an id names, and its id as Telegram's API knows it.
///
/// The friendly types and methods use Bot-API-style `i64` ids: users positive, basic groups negated, and channels
/// and supergroups as `-100` followed by the channel id. `PeerId` is the typed view of such an id:
/// [`from_bot_api`](PeerId::from_bot_api) reads one and [`bot_api`](PeerId::bot_api) writes it back, so code can
/// `match` on the kind instead of comparing against magic ranges. It converts into a [`PeerRef`](super::PeerRef),
/// so any method that takes a chat takes one.
///
/// ```
/// use vaegram::types::PeerId;
///
/// assert_eq!(PeerId::from_bot_api(-1001234567890), PeerId::Channel(1234567890));
/// assert_eq!(PeerId::Chat(42).bot_api(), -42);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum PeerId {
    /// A user or bot.
    User(i64),
    /// A basic group, by its positive id.
    Chat(i64),
    /// A channel or supergroup, by its positive id.
    Channel(i64),
}

impl PeerId {
    /// Reads a Bot-API-style id: positive is a user, `-1000000000000` or less a channel, and any other id, 0
    /// included, a basic group.
    pub const fn from_bot_api(id: i64) -> PeerId {
        if id > 0 {
            PeerId::User(id)
        } else if id <= -CHANNEL_OFFSET {
            // Can't overflow: `id <= -CHANNEL_OFFSET` makes the result 0 or more and at most `i64::MAX`.
            PeerId::Channel(-CHANNEL_OFFSET - id)
        } else {
            // Can't overflow: `-CHANNEL_OFFSET < id <= 0`.
            PeerId::Chat(-id)
        }
    }

    /// The Bot-API-style id. An id from the server out of range wraps instead of panicking.
    pub const fn bot_api(self) -> i64 {
        match self {
            PeerId::User(id) => id,
            PeerId::Chat(id) => id.wrapping_neg(),
            PeerId::Channel(id) => (-CHANNEL_OFFSET).wrapping_sub(id),
        }
    }

    /// `"user"`, `"group"` or `"channel"`, as the storage engines label peers.
    #[cfg_attr(
        not(any(feature = "sqlite", feature = "redis", feature = "mongodb", feature = "postgres")),
        allow(dead_code)
    )]
    pub(crate) const fn kind(self) -> &'static str {
        match self {
            PeerId::User(_) => "user",
            PeerId::Chat(_) => "group",
            PeerId::Channel(_) => "channel",
        }
    }

    /// The channel id of a channel or supergroup, `None` for other peers.
    pub const fn channel(self) -> Option<i64> {
        match self {
            PeerId::Channel(id) => Some(id),
            _ => None,
        }
    }

    /// The channel's Bot-API id for a channel id a caller typed: `None` unless it is positive and fits the `-100…`
    /// form.
    pub(crate) const fn checked_channel(channel_id: i64) -> Option<i64> {
        if channel_id <= 0 {
            return None;
        }
        (-CHANNEL_OFFSET).checked_sub(channel_id)
    }
}

impl fmt::Display for PeerId {
    /// Writes the Bot-API-style id.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.bot_api().fmt(f)
    }
}

impl From<&base::Peer> for PeerId {
    fn from(peer: &base::Peer) -> PeerId {
        match peer {
            base::Peer::PeerUser(p) => PeerId::User(p.user_id),
            base::Peer::PeerChat(p) => PeerId::Chat(p.chat_id),
            base::Peer::PeerChannel(p) => PeerId::Channel(p.channel_id),
        }
    }
}

impl From<PeerId> for base::Peer {
    fn from(id: PeerId) -> base::Peer {
        match id {
            PeerId::User(user_id) => base::Peer::PeerUser(types::PeerUser { user_id }),
            PeerId::Chat(chat_id) => base::Peer::PeerChat(types::PeerChat { chat_id }),
            PeerId::Channel(channel_id) => base::Peer::PeerChannel(types::PeerChannel { channel_id }),
        }
    }
}

impl From<PeerId> for i64 {
    fn from(id: PeerId) -> i64 {
        id.bot_api()
    }
}

#[cfg(test)]
mod tests {
    use super::{CHANNEL_OFFSET, PeerId};

    #[test]
    fn bot_api_ids_round_trip_by_kind() {
        let cases = [
            (7, PeerId::User(7)),
            (-7, PeerId::Chat(7)),
            (0, PeerId::Chat(0)),
            (-CHANNEL_OFFSET + 1, PeerId::Chat(CHANNEL_OFFSET - 1)),
            (-CHANNEL_OFFSET, PeerId::Channel(0)),
            (-1001234567890, PeerId::Channel(1234567890)),
        ];
        for (bot_api, id) in cases {
            assert_eq!(PeerId::from_bot_api(bot_api), id, "{bot_api}");
            assert_eq!(id.bot_api(), bot_api, "{id:?}");
        }
        assert_eq!(PeerId::Channel(5).to_string(), "-1000000000005");
    }

    #[test]
    fn extremes_do_not_panic() {
        for id in [i64::MIN, i64::MAX, -(CHANNEL_OFFSET + 1)] {
            let _ = PeerId::from_bot_api(id).bot_api();
        }
        assert_eq!(PeerId::from_bot_api(i64::MIN), PeerId::Channel(i64::MAX - CHANNEL_OFFSET + 1));
        assert_eq!(PeerId::Channel(i64::MAX).bot_api(), (-CHANNEL_OFFSET).wrapping_sub(i64::MAX));
        assert_eq!(PeerId::checked_channel(0), None);
        assert_eq!(PeerId::checked_channel(-5), None);
        assert_eq!(PeerId::checked_channel(i64::MAX), None);
        assert_eq!(PeerId::checked_channel(5), Some(-1000000000005));
    }
}
