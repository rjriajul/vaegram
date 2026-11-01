use crate::raw::base;
use crate::types::Sticker;

/// What a business account shows in an empty private chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusinessIntro {
    pub title: String,
    pub text: String,
    pub sticker: Option<Sticker>,
}

impl BusinessIntro {
    pub(crate) fn parse(raw: base::BusinessIntro) -> BusinessIntro {
        let base::BusinessIntro::BusinessIntro(i) = raw;
        let i = *i;
        BusinessIntro { title: i.title, text: i.description, sticker: i.sticker.as_ref().and_then(Sticker::parse) }
    }
}

#[cfg(test)]
mod tests {
    use super::BusinessIntro;
    use crate::raw::{base, types};

    fn sticker(id: i64) -> base::Document {
        let attribute = types::DocumentAttributeSticker {
            mask: false,
            alt: "👋".into(),
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

    #[test]
    fn reads_title_text_and_sticker() {
        let intro = |sticker| {
            let raw = types::BusinessIntro { title: "Hi".into(), description: "We answer fast".into(), sticker };
            BusinessIntro::parse(base::BusinessIntro::BusinessIntro(raw.into()))
        };
        let with = intro(Some(sticker(4)));
        assert_eq!((with.title.as_str(), with.text.as_str()), ("Hi", "We answer fast"));
        assert_eq!(with.sticker.map(|s| s.id), Some(4));
        assert_eq!(intro(None).sticker, None);
    }
}
