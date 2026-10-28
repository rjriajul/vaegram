use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::Sticker;

impl Client {
    pub async fn get_custom_emoji_stickers(&self, custom_emoji_ids: &[i64]) -> Result<Vec<Sticker>> {
        let request = functions::messages::GetCustomEmojiDocuments { document_id: custom_emoji_ids.to_vec() };
        Ok(self.invoke(&request).await?.iter().filter_map(Sticker::parse).collect())
    }
}
