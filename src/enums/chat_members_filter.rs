use crate::raw::{base, types};

/// Which members `get_chat_members` lists. Basic groups ignore the filter.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ChatMembersFilter {
    /// All members whose name matches the query. An empty query matches everyone.
    #[default]
    Search,
    /// Banned members whose name matches the query.
    Banned,
    /// Restricted members whose name matches the query.
    Restricted,
    /// Bots.
    Bots,
    /// Recently active members.
    Recent,
    /// Admins.
    Administrators,
}

impl ChatMembersFilter {
    pub(crate) fn write(self, q: String) -> base::ChannelParticipantsFilter {
        use base::ChannelParticipantsFilter as F;
        match self {
            Self::Search => F::ChannelParticipantsSearch(Box::new(types::ChannelParticipantsSearch { q })),
            Self::Banned => F::ChannelParticipantsKicked(Box::new(types::ChannelParticipantsKicked { q })),
            Self::Restricted => F::ChannelParticipantsBanned(Box::new(types::ChannelParticipantsBanned { q })),
            Self::Bots => F::ChannelParticipantsBots,
            Self::Recent => F::ChannelParticipantsRecent,
            Self::Administrators => F::ChannelParticipantsAdmins,
        }
    }
}
