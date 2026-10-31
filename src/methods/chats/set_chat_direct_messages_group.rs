use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// `chat_id` is the channel, not its direct messages chat.
    pub async fn set_chat_direct_messages_group(
        &self,
        chat_id: impl Into<PeerRef>,
        is_enabled: bool,
        paid_message_star_count: i64,
    ) -> Result<()> {
        if paid_message_star_count < 0 {
            return Err(Error::InvalidArgument("paid_message_star_count can't be negative"));
        }
        let request = functions::channels::UpdatePaidMessagesPrice {
            broadcast_messages_allowed: is_enabled,
            channel: self.resolve_channel(&chat_id.into()).await?,
            send_paid_messages_stars: paid_message_star_count,
        };
        self.invoke(&request).await?;
        Ok(())
    }
}
