use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::Gift;

impl Client {
    /// Lists the gifts in the gift shop, including sold-out ones (see [`Gift::is_sold_out`]).
    ///
    /// Unique gifts are left out. A gift that doesn't parse is skipped.
    pub async fn get_available_gifts(&self) -> Result<Vec<Gift>> {
        let gifts = self.invoke(&functions::payments::GetStarGifts { hash: 0 }).await?;
        let base::payments::StarGifts::StarGifts(gifts) = gifts else {
            return Err(Error::Unexpected("payments.getStarGifts returned no gift list"));
        };
        let entities = Entities::new(self, gifts.users, gifts.chats);
        Ok(gifts
            .gifts
            .iter()
            .filter_map(|g| match g {
                base::StarGift::StarGift(g) => Gift::parse(g, &entities),
                base::StarGift::StarGiftUnique(_) => None,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::StickerType;
    use crate::raw::{Serializable, base, functions, types};

    /// A gift's sticker as Telegram sends it: an animated custom emoji document, with no `documentAttributeSticker`.
    fn custom_emoji_sticker(id: i64, alt: &str) -> base::Document {
        let emoji = types::DocumentAttributeCustomEmoji {
            free: false,
            text_color: false,
            alt: alt.into(),
            stickerset: base::InputStickerSet::InputStickerSetID(Box::new(types::InputStickerSetID {
                id: 77,
                access_hash: 78,
            })),
        };
        let size = types::DocumentAttributeImageSize { w: 512, h: 512 };
        let name = types::DocumentAttributeFilename { file_name: "AnimatedSticker.tgs".into() };
        base::Document::Document(Box::new(types::Document {
            id,
            access_hash: 5,
            file_reference: vec![1, 2],
            date: 1_700_000_000,
            mime_type: "application/x-tgsticker".into(),
            size: 30_000,
            thumbs: None,
            video_thumbs: None,
            dc_id: 2,
            attributes: vec![
                base::DocumentAttribute::DocumentAttributeImageSize(size),
                base::DocumentAttribute::DocumentAttributeCustomEmoji(Box::new(emoji)),
                base::DocumentAttribute::DocumentAttributeFilename(Box::new(name)),
            ],
        }))
    }

    fn gift(id: i64, stars: i64, convert_stars: i64, limited: bool) -> base::StarGift {
        base::StarGift::StarGift(Box::new(types::StarGift {
            limited,
            sold_out: limited,
            birthday: false,
            require_premium: false,
            limited_per_user: false,
            peer_color_available: false,
            auction: false,
            id,
            sticker: custom_emoji_sticker(id + 1, "💝"),
            stars,
            availability_remains: limited.then_some(0),
            availability_total: limited.then_some(1_000),
            availability_resale: None,
            convert_stars,
            first_sale_date: limited.then_some(1_700_000_000),
            last_sale_date: limited.then_some(1_700_000_100),
            upgrade_stars: None,
            resell_min_stars: None,
            title: None,
            released_by: None,
            per_user_total: None,
            per_user_remains: None,
            locked_until_date: None,
            auction_slug: None,
            gifts_per_round: None,
            auction_start_date: None,
            upgrade_variants: None,
            background: None,
        }))
    }

    #[tokio::test]
    async fn reads_gifts_whose_stickers_are_custom_emoji() {
        let shop = types::payments::StarGifts {
            hash: 1,
            gifts: vec![gift(10, 15, 13, false), gift(20, 500, 425, true)],
            chats: Vec::new(),
            users: Vec::new(),
        };
        let reply = base::payments::StarGifts::StarGifts(Box::new(shop)).to_bytes();
        let (client, mut sent) = Client::scripted(vec![Ok(reply)]);

        let gifts = client.get_available_gifts().await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), functions::payments::GetStarGifts { hash: 0 }.to_bytes());
        let read: Vec<_> = gifts.iter().map(|g| (g.id, g.star_count, g.convert_star_count, g.is_sold_out)).collect();
        assert_eq!(read, [(10, 15, 13, false), (20, 500, 425, true)]);
        assert_eq!((gifts[0].sticker.kind, gifts[0].sticker.emoji.as_deref()), (StickerType::CustomEmoji, Some("💝")));
        assert!(gifts[0].sticker.is_animated);
    }
}
