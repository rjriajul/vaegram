use crate::raw::base;

/// A kind of chat a bot's attachment menu entry can be used in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AttachMenuPeerType {
    /// The private chat with the bot that owns the entry.
    SameBot,
    /// Private chats with other bots.
    Bot,
    /// Private chats with users.
    Private,
    /// Basic groups and supergroups.
    Group,
    /// Broadcast channels.
    Channel,
}

impl AttachMenuPeerType {
    pub(crate) fn parse(kind: &base::AttachMenuPeerType) -> Self {
        use base::AttachMenuPeerType as T;
        match kind {
            T::AttachMenuPeerTypeSameBotPM => Self::SameBot,
            T::AttachMenuPeerTypeBotPM => Self::Bot,
            T::AttachMenuPeerTypePM => Self::Private,
            T::AttachMenuPeerTypeChat => Self::Group,
            T::AttachMenuPeerTypeBroadcast => Self::Channel,
        }
    }
}
