use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{Message, PeerRef};

impl Client {
    pub async fn get_discussion_message(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> Result<Message> {
        let peer = self.resolve_peer(chat_id).await?;
        let request = functions::messages::GetDiscussionMessage { peer, msg_id: message_id };
        let base::messages::DiscussionMessage::DiscussionMessage(found) = self.invoke(&request).await?;
        let entities = Entities::new(self, found.users, found.chats);
        found
            .messages
            .into_iter()
            .find_map(|m| Message::from_raw(self, m, &entities))
            .ok_or(Error::Unexpected("Telegram sent no discussion message"))
    }
}
