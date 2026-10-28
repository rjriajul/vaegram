use crate::Client;
use crate::errors::Result;
use crate::raw::base;
use crate::types::PeerRef;

impl Client {
    pub async fn delete_chat_sticker_set(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        self.set_chat_stickers(&chat_id.into(), base::InputStickerSet::InputStickerSetEmpty).await
    }
}
