use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::{Chat, InlineQueryResult, Message, SentGuestMessage, User};

/// A message in a chat the bot isn't in; `answer` is the only way to respond there.
#[derive(Debug, Clone)]
pub struct GuestMessage {
    pub message: Message,
    pub query_id: i64,
    pub caller_user: Option<User>,
    pub caller_chat: Option<Chat>,
}

impl GuestMessage {
    pub(crate) fn parse(client: &Client, raw: types::UpdateBotGuestChatQuery, entities: &Entities) -> Option<Self> {
        let base::Message::Message(m) = raw.message else { return None };
        let (mut caller_user, mut caller_chat) = (None, None);
        match &m.guestchat_via_from {
            Some(base::Peer::PeerUser(p)) => caller_user = Some(User::parse(p.user_id, entities)),
            Some(peer) => caller_chat = Some(Chat::parse(peer, entities, false)),
            None => {}
        }
        let message = Message::parse(client, m, entities);
        Some(GuestMessage { message, query_id: raw.query_id, caller_user, caller_chat })
    }

    pub(crate) fn client(&self) -> &Client {
        self.message.client()
    }

    pub async fn answer(&self, result: impl Into<InlineQueryResult>) -> Result<SentGuestMessage> {
        self.client().answer_guest_query(self.query_id, result).await
    }
}
