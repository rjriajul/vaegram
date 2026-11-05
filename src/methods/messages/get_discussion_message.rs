use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{Message, PeerRef};

impl Client {
    /// Returns the message in the discussion group that belongs to the channel post `message_id`.
    ///
    /// Fails with `Error::Unexpected` if Telegram's answer holds no message that can be read.
    pub async fn get_discussion_message(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> Result<Message> {
        let peer = self.resolve_peer(chat_id).await?;
        let request = functions::messages::GetDiscussionMessage { peer, msg_id: message_id };
        let base::messages::DiscussionMessage::DiscussionMessage(found) = self.invoke(&request).await?;
        let entities = Entities::new(self, found.users, found.chats);
        let message = found.messages.into_iter().find_map(|m| Message::from_raw(self, m, &entities));
        let mut message = message.ok_or(Error::Unexpected("Telegram sent no discussion message"))?;
        self.prefetch(std::slice::from_mut(&mut message)).await;
        Ok(message)
    }
}
