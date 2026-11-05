use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::EmojiStatus;

impl Client {
    /// Returns the default emoji statuses a channel can use.
    pub async fn get_default_channel_emoji_statuses(&self) -> Result<Vec<EmojiStatus>> {
        Ok(EmojiStatus::parse_list(self.invoke(&functions::account::GetChannelDefaultEmojiStatuses { hash: 0 }).await?))
    }
}
