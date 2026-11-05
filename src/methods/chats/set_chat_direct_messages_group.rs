use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Turns the direct messages chat of a channel on or off.
    ///
    /// `chat_id` is the channel, not its direct messages chat. `paid_message_star_count` is the Stars price per
    /// message, and 0 makes messages free. A negative price fails with `InvalidArgument`.
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
