use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::ChatAction;
use crate::raw::{base, types};
use crate::types::{Chat, User};

/// Someone in a chat shows an activity, such as typing or recording a voice message, or stopped showing one.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct ChatActionUpdated {
    /// The chat the activity is shown in.
    pub chat: Chat,
    /// The user doing it. `None` when a chat does it, which is then in `sender_chat`.
    pub from_user: Option<User>,
    /// Set instead of `from_user` when the activity is shown on behalf of a chat.
    pub sender_chat: Option<Chat>,
    /// The activity; [`ChatAction::Cancel`] when it stopped.
    pub action: ChatAction,
    /// Upload progress in percent, for upload actions and history imports. `None` for the rest.
    pub progress: Option<i32>,
    /// Id of the forum topic or thread the activity is in. `None` outside of one.
    pub message_thread_id: Option<i32>,
    client: Client,
}

impl ChatActionUpdated {
    fn new(
        client: &Client,
        chat: Chat,
        from: &base::Peer,
        action: &base::SendMessageAction,
        message_thread_id: Option<i32>,
        entities: &Entities,
    ) -> Option<Self> {
        let (action, progress) = ChatAction::parse(action)?;
        let (from_user, sender_chat) = match from {
            base::Peer::PeerUser(p) => (Some(User::parse(p.user_id, entities)), None),
            peer => (None, Some(Chat::parse(peer, entities, false))),
        };
        let client = client.clone();
        Some(ChatActionUpdated { chat, from_user, sender_chat, action, progress, message_thread_id, client })
    }

    /// `None` for activities [`ChatAction`] has no variant for, such as streamed drafts and emoji interactions.
    pub(crate) fn parse_user(client: &Client, raw: &types::UpdateUserTyping, entities: &Entities) -> Option<Self> {
        let peer = base::Peer::PeerUser(types::PeerUser { user_id: raw.user_id });
        let chat = Chat::parse(&peer, entities, false);
        Self::new(client, chat, &peer, &raw.action, raw.top_msg_id, entities)
    }

    pub(crate) fn parse_chat(client: &Client, raw: &types::UpdateChatUserTyping, entities: &Entities) -> Option<Self> {
        let chat = Chat::parse(&base::Peer::PeerChat(types::PeerChat { chat_id: raw.chat_id }), entities, false);
        Self::new(client, chat, &raw.from_id, &raw.action, None, entities)
    }

    pub(crate) fn parse_channel(
        client: &Client,
        raw: &types::UpdateChannelUserTyping,
        entities: &Entities,
    ) -> Option<Self> {
        let peer = base::Peer::PeerChannel(types::PeerChannel { channel_id: raw.channel_id });
        let chat = Chat::parse(&peer, entities, false);
        Self::new(client, chat, &raw.from_id, &raw.action, raw.top_msg_id, entities)
    }

    /// The [`Client`] that received this update.
    pub fn client(&self) -> &Client {
        &self.client
    }
}
