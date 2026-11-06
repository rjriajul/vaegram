use crate::types::{Chat, FailedToAddMember};

/// A basic group made by [`Client::create_group`](crate::Client::create_group), with the users left out of it.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CreatedGroup {
    /// The new group.
    pub chat: Chat,
    /// Users Telegram could not add, and why. Empty when everyone was added.
    pub failed_members: Vec<FailedToAddMember>,
}
