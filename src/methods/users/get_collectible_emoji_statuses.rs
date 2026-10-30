use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::EmojiStatus;

impl Client {
    pub async fn get_collectible_emoji_statuses(&self) -> Result<Vec<EmojiStatus>> {
        Ok(EmojiStatus::parse_list(self.invoke(&functions::account::GetCollectibleEmojiStatuses { hash: 0 }).await?))
    }
}
