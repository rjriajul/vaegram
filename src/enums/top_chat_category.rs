/// Which list of frequently used chats [`Client::get_top_chats`](crate::Client::get_top_chats) returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TopChatCategory {
    /// Users you message most.
    Users,
    /// Bots you message most.
    Bots,
    /// Bots you use most in inline mode.
    InlineBots,
    /// Users you call most.
    Calls,
    /// Chats you forward messages to most.
    ForwardChats,
    /// Groups you use most.
    Groups,
    /// Channels you use most.
    Channels,
    /// Bots whose Mini Apps you open most.
    WebAppBots,
    GuestBots,
}
