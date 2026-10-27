use crate::raw::{base, types};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum ChatMembersFilter {
    #[default]
    Search,
    Banned,
    Restricted,
    Bots,
    Recent,
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
