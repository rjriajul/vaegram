use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::EmojiStatus;

impl Client {
    pub async fn get_recent_emoji_statuses(&self) -> Result<Vec<EmojiStatus>> {
        Ok(EmojiStatus::parse_list(self.invoke(&functions::account::GetRecentEmojiStatuses { hash: 0 }).await?))
    }
}
