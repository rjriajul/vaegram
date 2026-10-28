use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::Document;

impl Client {
    /// Custom emoji usable as forum topic icons; a document's `id` is its custom emoji id.
    pub async fn get_forum_topic_icon_stickers(&self) -> Result<Vec<Document>> {
        let request = functions::messages::GetStickerSet {
            stickerset: base::InputStickerSet::InputStickerSetEmojiDefaultTopicIcons,
            hash: 0,
        };
        let base::messages::StickerSet::StickerSet(set) = self.invoke(&request).await? else {
            return Err(Error::Unexpected("messages.getStickerSet returned stickerSetNotModified"));
        };
        Ok(set.documents.iter().filter_map(Document::parse).collect())
    }
}
