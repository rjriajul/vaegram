use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Message, PeerRef};

impl Client {
    /// Returns the last `limit` (1-20) messages of the channel the user added to their profile.
    pub async fn get_user_personal_chat_messages(
        &self,
        user_id: impl Into<PeerRef>,
        limit: i32,
    ) -> Result<Vec<Message>> {
        let user_id = self.resolve_user(&user_id.into()).await?;
        let request = functions::messages::GetPersonalChannelHistory { user_id, limit, max_id: 0, min_id: 0, hash: 0 };
        let (messages, entities) = Message::unpack(self, self.invoke(&request).await?);
        Ok(messages.into_iter().filter_map(|m| Message::from_raw(self, m, &entities)).collect())
    }
}
