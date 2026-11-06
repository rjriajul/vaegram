/// A member's standing in a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ChatMemberStatus {
    /// The chat's owner.
    Owner,
    /// An administrator other than the owner.
    Administrator,
    /// A regular member.
    Member,
    /// Has some permissions taken away but can still view the chat. A restricted user can be outside the chat,
    /// see [`ChatMember::is_member`](crate::types::ChatMember::is_member).
    Restricted,
    /// Not in the chat.
    Left,
    /// Barred from viewing the chat.
    Banned,
}
