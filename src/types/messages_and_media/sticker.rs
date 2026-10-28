use crate::enums::StickerType;
use crate::raw::base;
use crate::types::{Document, FileLocation, MaskPosition};

use super::input_document;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sticker {
    pub id: i64,
    pub kind: StickerType,
    pub mime_type: String,
    pub file_size: i64,
    pub date: i32,
    pub width: i32,
    pub height: i32,
    pub is_animated: bool,
    pub is_video: bool,
    pub emoji: Option<String>,
    pub mask_position: Option<MaskPosition>,
    pub custom_emoji_id: Option<i64>,
    pub needs_repainting: bool,
    pub(crate) location: FileLocation,
}

impl Sticker {
    pub(crate) fn parse(document: &base::Document) -> Option<Sticker> {
        let base::Document::Document(raw) = document else { return None };
        Sticker::from_document(Document::parse(document)?, &raw.attributes)
    }

    /// `None` when the attributes mark neither a sticker nor a custom emoji.
    pub(crate) fn from_document(document: Document, attributes: &[base::DocumentAttribute]) -> Option<Sticker> {
        let (mut sticker, mut image, mut video) = (None, None, None);
        for attribute in attributes {
            match attribute {
                base::DocumentAttribute::DocumentAttributeSticker(a) => {
                    let kind = if a.mask { StickerType::Mask } else { StickerType::Regular };
                    sticker = Some((kind, &a.alt, a.mask_coords.as_ref().and_then(MaskPosition::parse), false));
                }
                base::DocumentAttribute::DocumentAttributeCustomEmoji(a) => {
                    sticker = Some((StickerType::CustomEmoji, &a.alt, None, a.text_color));
                }
                base::DocumentAttribute::DocumentAttributeImageSize(a) => image = Some((a.w, a.h)),
                base::DocumentAttribute::DocumentAttributeVideo(a) => video = Some((a.w, a.h)),
                _ => {}
            }
        }
        let (kind, emoji, mask_position, needs_repainting) = sticker?;
        let Document { id, mime_type, file_size, date, location, .. } = document;
        let (width, height) = image.or(video).unwrap_or_default();
        Some(Sticker {
            id,
            kind,
            is_animated: mime_type == "application/x-tgsticker",
            is_video: mime_type == "video/webm",
            mime_type,
            file_size,
            date,
            width,
            height,
            emoji: (!emoji.is_empty()).then(|| emoji.clone()),
            mask_position,
            custom_emoji_id: (kind == StickerType::CustomEmoji).then_some(id),
            needs_repainting,
            location,
        })
    }

    pub(crate) fn input_document(&self) -> base::InputDocument {
        input_document(&self.location)
    }
}

impl From<&Sticker> for FileLocation {
    fn from(sticker: &Sticker) -> Self {
        sticker.location.clone()
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::Sticker;
    use crate::enums::{MaskPointType, StickerType};
    use crate::raw::{base, types};

    pub(crate) fn document(id: i64, mime_type: &str, attributes: Vec<base::DocumentAttribute>) -> base::Document {
        base::Document::Document(Box::new(types::Document {
            id,
            access_hash: 2,
            file_reference: vec![3],
            date: 4,
            mime_type: mime_type.into(),
            size: 5,
            thumbs: None,
            video_thumbs: None,
            dc_id: 2,
            attributes,
        }))
    }

    pub(crate) fn sticker_attribute(alt: &str, mask_coords: Option<base::MaskCoords>) -> base::DocumentAttribute {
        base::DocumentAttribute::DocumentAttributeSticker(Box::new(types::DocumentAttributeSticker {
            mask: mask_coords.is_some(),
            alt: alt.into(),
            stickerset: base::InputStickerSet::InputStickerSetEmpty,
            mask_coords,
        }))
    }

    fn size(w: i32, h: i32) -> base::DocumentAttribute {
        base::DocumentAttribute::DocumentAttributeImageSize(Box::new(types::DocumentAttributeImageSize { w, h }))
    }

    #[test]
    fn reads_regular_mask_and_video_stickers() {
        let regular = Sticker::parse(&document(1, "image/webp", vec![sticker_attribute("😀", None), size(512, 480)]));
        let regular = regular.unwrap();
        assert_eq!((regular.kind, regular.emoji.as_deref()), (StickerType::Regular, Some("😀")));
        assert_eq!((regular.width, regular.height, regular.is_animated, regular.is_video), (512, 480, false, false));
        assert_eq!((regular.custom_emoji_id, regular.mask_position), (None, None));

        let coords = types::MaskCoords { n: 1, x: 0.0, y: 0.0, zoom: 1.0 };
        let mask = sticker_attribute("", Some(base::MaskCoords::MaskCoords(Box::new(coords))));
        let mask = Sticker::parse(&document(2, "image/webp", vec![mask])).unwrap();
        assert_eq!((mask.kind, mask.emoji), (StickerType::Mask, None));
        assert_eq!(mask.mask_position.map(|p| p.point), Some(MaskPointType::Eyes));

        let video = base::DocumentAttribute::DocumentAttributeVideo(Box::new(types::DocumentAttributeVideo {
            round_message: false,
            supports_streaming: false,
            nosound: true,
            duration: 3.0,
            w: 512,
            h: 512,
            preload_prefix_size: None,
            video_start_ts: None,
            video_codec: None,
        }));
        let video = Sticker::parse(&document(3, "video/webm", vec![video, sticker_attribute("🎉", None)])).unwrap();
        assert_eq!((video.width, video.is_video, video.is_animated), (512, true, false));
    }

    #[test]
    fn reads_custom_emoji_and_skips_other_files() {
        let emoji = types::DocumentAttributeCustomEmoji {
            free: false,
            text_color: true,
            alt: "⭐".into(),
            stickerset: base::InputStickerSet::InputStickerSetEmpty,
        };
        let emoji = base::DocumentAttribute::DocumentAttributeCustomEmoji(Box::new(emoji));
        let emoji = Sticker::parse(&document(9, "application/x-tgsticker", vec![emoji])).unwrap();
        assert_eq!((emoji.kind, emoji.custom_emoji_id), (StickerType::CustomEmoji, Some(9)));
        assert!(emoji.needs_repainting && emoji.is_animated);
        let base::InputDocument::InputDocument(input) = emoji.input_document() else { panic!("no input document") };
        assert_eq!((input.id, input.access_hash, input.file_reference.as_slice()), (9, 2, &[3][..]));

        assert!(Sticker::parse(&document(4, "application/pdf", vec![size(1, 1)])).is_none());
        assert!(Sticker::parse(&base::Document::DocumentEmpty(Box::new(types::DocumentEmpty { id: 4 }))).is_none());
    }
}
