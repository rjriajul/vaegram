/// The kind of a chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatType {
    /// A private chat with a user.
    Private,
    /// A private chat with a bot.
    Bot,
    /// A basic group.
    Group,
    /// A supergroup.
    Supergroup,
    /// A broadcast channel.
    Channel,
}
