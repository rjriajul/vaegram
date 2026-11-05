/// A member's standing in a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatMemberStatus {
    /// Created the chat.
    Owner,
    /// An admin.
    Administrator,
    /// A regular member.
    Member,
    /// Has some permissions taken away but can still view the chat.
    Restricted,
    /// Not in the chat.
    Left,
    /// Barred from viewing the chat.
    Banned,
}
