use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Sticker;

impl Client {
    /// Returns the stickers Telegram suggests for an emoji.
    ///
    /// [`search_stickers`](Client::search_stickers) searches by a text query instead.
    pub async fn get_stickers(&self, emoji: impl Into<String>) -> Result<Vec<Sticker>> {
        let request = functions::messages::GetStickers { emoticon: emoji.into(), hash: 0 };
        Ok(match self.invoke(&request).await? {
            base::messages::Stickers::Stickers(r) => r.stickers.iter().filter_map(Sticker::parse).collect(),
            base::messages::Stickers::StickersNotModified => Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    fn sticker(id: i64) -> base::Document {
        let attribute = types::DocumentAttributeSticker {
            mask: false,
            alt: "x".into(),
            stickerset: base::InputStickerSet::InputStickerSetEmpty,
            mask_coords: None,
        };
        base::Document::Document(Box::new(types::Document {
            id,
            access_hash: 2,
            file_reference: vec![3],
            date: 4,
            mime_type: "image/webp".into(),
            size: 5,
            thumbs: None,
            video_thumbs: None,
            dc_id: 2,
            attributes: vec![base::DocumentAttribute::DocumentAttributeSticker(Box::new(attribute))],
        }))
    }

    #[tokio::test]
    async fn lists_the_stickers_and_an_unchanged_answer_is_empty() {
        let list = types::messages::Stickers { hash: 6, stickers: vec![sticker(1), sticker(2)] };
        let found = base::messages::Stickers::Stickers(Box::new(list));
        let unchanged = base::messages::Stickers::StickersNotModified;
        let (client, mut sent) = Client::scripted(vec![Ok(found.to_bytes()), Ok(unchanged.to_bytes())]);
        let stickers = client.get_stickers("🐱").await.unwrap();
        assert_eq!(stickers.iter().map(|s| s.id).collect::<Vec<_>>(), [1, 2]);
        assert!(client.get_stickers("🐱").await.unwrap().is_empty());
        let expected = functions::messages::GetStickers { emoticon: "🐱".into(), hash: 0 };
        for _ in 0..2 {
            assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
        }
    }
}
