use super::gift::sticker;
use crate::raw::base;
use crate::types::Sticker;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiftCollection {
    pub id: i32,
    pub name: String,
    pub icon: Option<Sticker>,
    pub gift_count: i32,
}

impl GiftCollection {
    pub(crate) fn parse(raw: base::StarGiftCollection) -> GiftCollection {
        let base::StarGiftCollection::StarGiftCollection(raw) = raw;
        GiftCollection {
            id: raw.collection_id,
            icon: raw.icon.as_ref().and_then(sticker),
            name: raw.title,
            gift_count: raw.gifts_count,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::GiftCollection;
    use crate::raw::{base, types};
    use crate::types::messages_and_media::media::tests::document_media;

    fn collection(id: i32, icon: Option<base::Document>) -> base::StarGiftCollection {
        base::StarGiftCollection::StarGiftCollection(Box::new(types::StarGiftCollection {
            collection_id: id,
            title: "Best".into(),
            icon,
            gifts_count: 2,
            hash: 0,
        }))
    }

    #[test]
    fn maps_a_collection_and_its_icon() {
        let plain = GiftCollection::parse(collection(4, None));
        assert_eq!(plain, GiftCollection { id: 4, name: "Best".into(), icon: None, gift_count: 2 });

        let sticker = base::DocumentAttribute::DocumentAttributeSticker(Box::new(types::DocumentAttributeSticker {
            mask: false,
            alt: String::new(),
            stickerset: base::InputStickerSet::InputStickerSetEmpty,
            mask_coords: None,
        }));
        let base::MessageMedia::MessageMediaDocument(media) = document_media("image/webp", vec![sticker]) else {
            panic!("expected a document")
        };
        let with_icon = GiftCollection::parse(collection(4, media.document));
        assert!(with_icon.icon.is_some());
    }
}
