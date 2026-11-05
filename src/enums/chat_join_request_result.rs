use crate::raw::base;

/// How a bot answers a chat join request query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatJoinRequestResult {
    /// Lets the user in.
    Approve,
    /// Turns the request down.
    Decline,
    /// Leaves the request pending for an admin to decide.
    Queue,
}

impl ChatJoinRequestResult {
    pub(crate) fn write(self) -> base::JoinChatBotResult {
        match self {
            ChatJoinRequestResult::Approve => base::JoinChatBotResult::JoinChatBotResultApproved,
            ChatJoinRequestResult::Decline => base::JoinChatBotResult::JoinChatBotResultDeclined,
            ChatJoinRequestResult::Queue => base::JoinChatBotResult::JoinChatBotResultQueued,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ChatJoinRequestResult;
    use crate::raw::base;

    #[test]
    fn maps_to_raw_results() {
        assert!(matches!(ChatJoinRequestResult::Approve.write(), base::JoinChatBotResult::JoinChatBotResultApproved));
        assert!(matches!(ChatJoinRequestResult::Decline.write(), base::JoinChatBotResult::JoinChatBotResultDeclined));
        assert!(matches!(ChatJoinRequestResult::Queue.write(), base::JoinChatBotResult::JoinChatBotResultQueued));
    }
}
