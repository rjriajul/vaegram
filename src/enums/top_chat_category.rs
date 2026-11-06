/// Which list of frequently used chats [`Client::get_top_chats`](crate::Client::get_top_chats) returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TopChatCategory {
    /// Users you message most.
    Users,
    /// Bots you chat with most in private.
    Bots,
    /// Bots you use most in inline mode.
    InlineBots,
    /// Users you call most.
    Calls,
    /// Users and chats you forward messages to most.
    ForwardChats,
    /// Groups you use most.
    Groups,
    /// Channels you use most.
    Channels,
    /// Bots whose Mini Apps you open most.
    WebAppBots,
    /// Bots you use most in guest chats.
    GuestBots,
}
