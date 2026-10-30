use crate::enums::StickerType;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::types::{FileLocation, Sticker};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StickerSet {
    pub id: i64,
    /// The short name in `t.me/addstickers/<name>` links, which the sticker set methods take.
    pub name: String,
    pub title: String,
    pub sticker_type: StickerType,
    pub stickers: Vec<Sticker>,
    /// The set's own thumbnail; without one, apps show the first sticker.
    pub thumbnail: Option<FileLocation>,
}

impl StickerSet {
    pub(crate) fn parse(set: base::messages::StickerSet) -> Result<StickerSet> {
        // Only a request with a non-zero hash can come back unmodified, and none of ours send one.
        let base::messages::StickerSet::StickerSet(set) = set else {
            return Err(Error::Unexpected("Telegram returned stickerSetNotModified"));
        };
        let types::messages::StickerSet { set, documents, .. } = *set;
        Ok(StickerSet::from_raw(&set, &documents))
    }

    /// Set lists carry a few cover stickers at most, or none for installed sets.
    pub(crate) fn parse_covered(set: &base::StickerSetCovered) -> StickerSet {
        match set {
            base::StickerSetCovered::StickerSetCovered(s) => Self::from_raw(&s.set, std::slice::from_ref(&s.cover)),
            base::StickerSetCovered::StickerSetMultiCovered(s) => Self::from_raw(&s.set, &s.covers),
            base::StickerSetCovered::StickerSetFullCovered(s) => Self::from_raw(&s.set, &s.documents),
            base::StickerSetCovered::StickerSetNoCovered(s) => Self::from_raw(&s.set, &[]),
        }
    }

    pub(crate) fn from_raw(set: &base::StickerSet, documents: &[base::Document]) -> StickerSet {
        let base::StickerSet::StickerSet(set) = set;
        let sticker_type = match (set.masks, set.emojis) {
            (true, _) => StickerType::Mask,
            (_, true) => StickerType::CustomEmoji,
            _ => StickerType::Regular,
        };
        StickerSet {
            id: set.id,
            name: set.short_name.clone(),
            title: set.title.clone(),
            sticker_type,
            stickers: documents.iter().filter_map(Sticker::parse).collect(),
            thumbnail: thumbnail(set),
        }
    }
}

fn thumbnail(set: &types::StickerSet) -> Option<FileLocation> {
    let (Some(thumbs), Some(thumb_version)) = (&set.thumbs, set.thumb_version) else { return None };
    let file_size = thumbs
        .iter()
        .filter_map(|t| match t {
            base::PhotoSize::PhotoSize(s) => Some(s.size),
            base::PhotoSize::PhotoSizeProgressive(s) => s.sizes.iter().max().copied(),
            _ => None,
        })
        .max();
    let stickerset = types::InputStickerSetID { id: set.id, access_hash: set.access_hash };
    let stickerset = base::InputStickerSet::InputStickerSetID(Box::new(stickerset));
    Some(FileLocation {
        location: base::InputFileLocation::InputStickerSetThumb(Box::new(types::InputStickerSetThumb {
            stickerset,
            thumb_version,
        })),
        dc_id: set.thumb_dc_id,
        file_size: file_size.map(i64::from),
    })
}

#[cfg(test)]
mod tests {
    use super::StickerSet;
    use crate::enums::StickerType;
    use crate::raw::{base, types};
    use crate::types::messages_and_media::sticker::tests::{document, sticker_attribute};

    fn set(masks: bool, emojis: bool, thumb_version: Option<i32>) -> base::messages::StickerSet {
        let thumb = types::PhotoSize { r#type: "s".into(), w: 100, h: 100, size: 900 };
        let set = types::StickerSet {
            archived: false,
            official: false,
            masks,
            emojis,
            text_color: false,
            channel_emoji_status: false,
            creator: true,
            installed_date: None,
            id: 10,
            access_hash: 11,
            title: "Cats".into(),
            short_name: "cats_by_bot".into(),
            thumbs: thumb_version.map(|_| vec![base::PhotoSize::PhotoSize(Box::new(thumb))]),
            thumb_dc_id: thumb_version.map(|_| 4),
            thumb_version,
            thumb_document_id: None,
            count: 2,
            hash: 0,
        };
        let documents = vec![
            document(1, "image/webp", vec![sticker_attribute("😺", None)]),
            document(2, "application/pdf", Vec::new()),
            document(3, "image/webp", vec![sticker_attribute("😸", None)]),
        ];
        base::messages::StickerSet::StickerSet(Box::new(types::messages::StickerSet {
            set: base::StickerSet::StickerSet(Box::new(set)),
            packs: Vec::new(),
            keywords: Vec::new(),
            documents,
        }))
    }

    #[test]
    fn reads_the_set_type_stickers_and_thumbnail() {
        let cats = StickerSet::parse(set(false, false, Some(7))).unwrap();
        assert_eq!((cats.id, cats.name.as_str(), cats.title.as_str()), (10, "cats_by_bot", "Cats"));
        assert_eq!(cats.sticker_type, StickerType::Regular);
        assert_eq!(cats.stickers.iter().map(|s| s.id).collect::<Vec<_>>(), [1, 3]);
        let thumbnail = cats.thumbnail.unwrap();
        assert_eq!((thumbnail.dc_id, thumbnail.file_size), (Some(4), Some(900)));
        let base::InputFileLocation::InputStickerSetThumb(thumb) = thumbnail.location else {
            panic!("not a sticker set thumbnail")
        };
        assert_eq!(thumb.thumb_version, 7);
        let set_id = types::InputStickerSetID { id: 10, access_hash: 11 };
        assert_eq!(thumb.stickerset, base::InputStickerSet::InputStickerSetID(Box::new(set_id)));

        let masks = StickerSet::parse(set(true, false, None)).unwrap();
        assert_eq!((masks.sticker_type, masks.thumbnail.is_none()), (StickerType::Mask, true));
        assert_eq!(StickerSet::parse(set(false, true, None)).unwrap().sticker_type, StickerType::CustomEmoji);
        assert!(StickerSet::parse(base::messages::StickerSet::StickerSetNotModified).is_err());
    }

    #[test]
    fn reads_covered_sets() {
        let base::messages::StickerSet::StickerSet(full) = set(false, true, None) else { panic!("no set") };
        let types::messages::StickerSet { set, mut documents, .. } = *full;
        let covered = types::StickerSetCovered { set: set.clone(), cover: documents.remove(0) };
        let covered = StickerSet::parse_covered(&base::StickerSetCovered::StickerSetCovered(Box::new(covered)));
        assert_eq!((covered.sticker_type, covered.stickers.len()), (StickerType::CustomEmoji, 1));
        let bare = base::StickerSetCovered::StickerSetNoCovered(Box::new(types::StickerSetNoCovered { set }));
        assert!(StickerSet::parse_covered(&bare).stickers.is_empty());
    }
}
