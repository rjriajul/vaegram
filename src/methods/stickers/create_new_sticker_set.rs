use crate::Client;
use crate::enums::StickerType;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InputSticker, PeerRef, StickerSet};

pub struct CreateNewStickerSet<'a> {
    client: &'a Client,
    user: PeerRef,
    name: String,
    title: String,
    stickers: Vec<InputSticker<'a>>,
    sticker_type: StickerType,
    needs_repainting: bool,
}

impl Client {
    /// Creates a sticker set owned by `user_id`. A bot's set names must end in `_by_<bot username>`.
    pub fn create_new_sticker_set<'a>(
        &'a self,
        user_id: impl Into<PeerRef>,
        name: impl Into<String>,
        title: impl Into<String>,
        stickers: impl IntoIterator<Item = InputSticker<'a>>,
    ) -> CreateNewStickerSet<'a> {
        CreateNewStickerSet {
            client: self,
            user: user_id.into(),
            name: name.into(),
            title: title.into(),
            stickers: stickers.into_iter().collect(),
            sticker_type: StickerType::Regular,
            needs_repainting: false,
        }
    }
}

impl CreateNewStickerSet<'_> {
    pub fn sticker_type(mut self, sticker_type: StickerType) -> Self {
        self.sticker_type = sticker_type;
        self
    }

    pub fn needs_repainting(mut self) -> Self {
        self.needs_repainting = true;
        self
    }

    async fn send(self) -> Result<StickerSet> {
        let client = self.client;
        let mut stickers = Vec::with_capacity(self.stickers.len());
        for sticker in self.stickers {
            stickers.push(client.input_sticker(&self.user, sticker).await?);
        }
        let request = functions::stickers::CreateStickerSet {
            masks: self.sticker_type == StickerType::Mask,
            emojis: self.sticker_type == StickerType::CustomEmoji,
            text_color: self.needs_repainting,
            user_id: client.resolve_user(&self.user).await?,
            title: self.title,
            short_name: self.name,
            thumb: None,
            stickers,
            software: None,
        };
        StickerSet::parse(client.invoke(&request).await?)
    }
}

into_future!(CreateNewStickerSet, StickerSet);
