use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::Chat;

/// Notice that a bot stopped streaming a message draft, sent as a stop-draft typing action in a private chat.
#[derive(Debug, Clone)]
pub struct MessageGenerationStopped {
    /// The private chat the draft was in.
    pub chat: Chat,
    /// Id of the thread the draft was in. `None` outside of a thread.
    pub message_thread_id: Option<i32>,
    /// Id of the stopped draft.
    pub draft_id: i64,
    client: Client,
}

impl MessageGenerationStopped {
    pub(crate) fn parse(
        client: &Client,
        raw: &types::UpdateUserTyping,
        entities: &Entities,
    ) -> Option<MessageGenerationStopped> {
        let base::SendMessageAction::SendMessageStopDraftAction(action) = &raw.action else {
            return None;
        };
        let peer = base::Peer::PeerUser(types::PeerUser { user_id: raw.user_id });
        Some(MessageGenerationStopped {
            chat: Chat::parse(&peer, entities, false),
            message_thread_id: raw.top_msg_id,
            draft_id: action.random_id,
            client: client.clone(),
        })
    }

    /// The [`Client`] that received this update.
    pub fn client(&self) -> &Client {
        &self.client
    }
}

#[cfg(test)]
mod tests {
    use super::MessageGenerationStopped;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};

    fn typing(action: base::SendMessageAction) -> types::UpdateUserTyping {
        types::UpdateUserTyping { user_id: 7, top_msg_id: Some(3), action }
    }

    #[test]
    fn maps_stop_draft_action() {
        let stop =
            base::SendMessageAction::SendMessageStopDraftAction(types::SendMessageStopDraftAction { random_id: 42 });
        let event = MessageGenerationStopped::parse(&Client::offline(), &typing(stop), &Entities::default()).unwrap();
        assert_eq!((event.chat.id, event.message_thread_id, event.draft_id), (7, Some(3), 42));
    }

    #[test]
    fn ignores_plain_typing() {
        let update = typing(base::SendMessageAction::SendMessageTypingAction);
        assert!(MessageGenerationStopped::parse(&Client::offline(), &update, &Entities::default()).is_none());
    }
}
