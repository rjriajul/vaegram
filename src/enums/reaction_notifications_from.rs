use crate::raw::base;

/// Whose reactions, or poll votes, raise a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum ReactionNotificationsFrom {
    /// Only people in your contacts.
    Contacts,
    /// Everyone.
    All,
}

impl ReactionNotificationsFrom {
    pub(crate) fn parse(raw: base::ReactionNotificationsFrom) -> ReactionNotificationsFrom {
        match raw {
            base::ReactionNotificationsFrom::ReactionNotificationsFromContacts => ReactionNotificationsFrom::Contacts,
            base::ReactionNotificationsFrom::ReactionNotificationsFromAll => ReactionNotificationsFrom::All,
        }
    }

    pub(crate) fn write(self) -> base::ReactionNotificationsFrom {
        match self {
            ReactionNotificationsFrom::Contacts => base::ReactionNotificationsFrom::ReactionNotificationsFromContacts,
            ReactionNotificationsFrom::All => base::ReactionNotificationsFrom::ReactionNotificationsFromAll,
        }
    }
}
