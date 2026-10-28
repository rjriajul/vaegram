use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;
use crate::utils::input_sticker_set;

impl Client {
    pub async fn set_chat_sticker_set(&self, chat_id: impl Into<PeerRef>, name: &str) -> Result<()> {
        self.set_chat_stickers(&chat_id.into(), input_sticker_set(name)).await
    }

    pub(super) async fn set_chat_stickers(&self, chat: &PeerRef, stickerset: base::InputStickerSet) -> Result<()> {
        let channel = self.resolve_channel(chat).await?;
        self.invoke(&functions::channels::SetStickers { channel, stickerset }).await?;
        Ok(())
    }
}
