use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{MessageViews, PeerRef};

impl Client {
    pub async fn get_messages_views(
        &self,
        chat_id: impl Into<PeerRef>,
        message_ids: &[i32],
    ) -> Result<Vec<MessageViews>> {
        if message_ids.is_empty() {
            return Err(Error::InvalidArgument("get_messages_views needs at least one message id"));
        }
        let request = functions::messages::GetMessagesViews {
            peer: self.resolve_peer(chat_id).await?,
            id: message_ids.to_vec(),
            increment: false,
        };
        let base::messages::MessageViews::MessageViews(found) = self.invoke(&request).await?;
        Ok(found.views.iter().map(MessageViews::parse).collect())
    }
}
