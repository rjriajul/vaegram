/// How a user came to be in a chat, as a service message tells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatJoinType {
    /// Another member added them.
    ByAdd,
    /// They followed an invite link.
    ByLink,
    /// Their join request was approved.
    ByRequest,
}
