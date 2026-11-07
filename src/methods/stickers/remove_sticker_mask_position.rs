use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Sticker, StickerSet};

impl Client {
    /// Asks Telegram to remove the position of a mask sticker and returns the updated set.
    ///
    /// Sends `stickers.changeSticker` with no emoji, mask position or keywords. That this clears the position, rather
    /// than leaving the sticker unchanged, hasn't been checked against Telegram: read `mask_position` in the returned
    /// set to see what it did.
    pub async fn remove_sticker_mask_position(&self, sticker: &Sticker) -> Result<StickerSet> {
        let request = functions::stickers::ChangeSticker {
            sticker: sticker.input_document(),
            emoji: None,
            mask_coords: None,
            keywords: None,
        };
        StickerSet::parse(self.invoke(&request).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::Sticker;

    #[tokio::test]
    async fn sends_change_sticker_without_mask_coords() {
        let attribute = types::DocumentAttributeSticker {
            mask: true,
            alt: String::new(),
            stickerset: base::InputStickerSet::InputStickerSetEmpty,
            mask_coords: None,
        };
        let document = types::Document {
            id: 1,
            access_hash: 2,
            file_reference: vec![3],
            date: 4,
            mime_type: "image/webp".into(),
            size: 5,
            thumbs: None,
            video_thumbs: None,
            dc_id: 2,
            attributes: vec![base::DocumentAttribute::DocumentAttributeSticker(Box::new(attribute))],
        };
        let sticker = Sticker::parse(&base::Document::Document(Box::new(document))).unwrap();
        let (client, mut rx) = Client::recording();
        let _ = client.remove_sticker_mask_position(&sticker).await;
        let request = functions::stickers::ChangeSticker {
            sticker: sticker.input_document(),
            emoji: None,
            mask_coords: None,
            keywords: None,
        };
        let body = rx.try_recv().expect("no request sent");
        assert_eq!(body, request.to_bytes());
        assert_eq!(u32::from_le_bytes(body[4..8].try_into().unwrap()), 0, "a flag is set");
    }
}
