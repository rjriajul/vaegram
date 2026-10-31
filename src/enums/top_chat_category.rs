#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TopChatCategory {
    Users,
    Bots,
    InlineBots,
    Calls,
    ForwardChats,
    Groups,
    Channels,
    WebAppBots,
    GuestBots,
}
