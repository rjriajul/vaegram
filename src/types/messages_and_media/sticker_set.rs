use crate::enums::StickerType;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::types::{FileLocation, Sticker, StickerSetRef};

/// A sticker set, mask set or custom emoji set.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct StickerSet {
    /// Telegram's id of the set.
    pub id: i64,
    /// The short name in `t.me/addstickers/<name>` links, which the sticker set methods take.
    pub name: String,
    /// Display title of the set.
    pub title: String,
    /// What the set holds: regular stickers, masks or custom emoji.
    pub sticker_type: StickerType,
    /// Stickers in the set. Sets from a list may hold only a few cover stickers, or none.
    pub stickers: Vec<Sticker>,
    /// The set's own thumbnail. `None` if the set has none, and apps then show the first sticker.
    pub thumbnail: Option<FileLocation>,
    pub(crate) access_hash: i64,
    // Telegram's hash of the set's contents, which a later `messages.getStickerSet` sends to skip an unchanged set.
    pub(crate) hash: i32,
}

impl StickerSet {
    pub(crate) fn parse(set: base::messages::StickerSet) -> Result<StickerSet> {
        // Only a request with a non-zero hash can come back unmodified, and the callers of this send none.
        StickerSet::parse_modified(set).ok_or(Error::Unexpected("Telegram returned stickerSetNotModified"))
    }

    /// `None` when Telegram answers that the set still has the hash the request sent.
    pub(crate) fn parse_modified(set: base::messages::StickerSet) -> Option<StickerSet> {
        let base::messages::StickerSet::StickerSet(set) = set else { return None };
        let types::messages::StickerSet { set, packs, documents, .. } = *set;
        let mut parsed = StickerSet::from_raw(&set, &documents);
        // Each pack is an emoji with the stickers it tags; a sticker's own alt is only the first of them.
        for sticker in &mut parsed.stickers {
            let emojis: Vec<String> = packs
                .iter()
                .filter(|base::StickerPack::StickerPack(pack)| pack.documents.contains(&sticker.id))
                .map(|base::StickerPack::StickerPack(pack)| pack.emoticon.clone())
                .collect();
            if !emojis.is_empty() {
                sticker.emojis = emojis;
            }
        }
        Some(parsed)
    }

    /// The stickers the set tags with `emoji`, in the set's order. An emoji's variation selector (U+FE0F) is
    /// ignored, so `"❤"` and `"❤️"` find the same stickers.
    pub fn stickers_for<'s>(&'s self, emoji: &'s str) -> impl Iterator<Item = &'s Sticker> + 's {
        self.stickers.iter().filter(move |sticker| sticker.emojis.iter().any(|e| same_emoji(e, emoji)))
    }

    /// A reference to this set, for [`get_sticker_set`](crate::Client::get_sticker_set) or
    /// [`StickerSource::from_set`](crate::types::StickerSource::from_set).
    pub fn set_ref(&self) -> StickerSetRef {
        StickerSetRef::from_id(self.id, self.access_hash)
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
        let mut stickers: Vec<Sticker> = documents.iter().filter_map(Sticker::parse).collect();
        for sticker in &mut stickers {
            sticker.set_name = Some(set.short_name.clone());
        }
        StickerSet {
            id: set.id,
            name: set.short_name.clone(),
            title: set.title.clone(),
            sticker_type,
            stickers,
            thumbnail: thumbnail(set),
            access_hash: set.access_hash,
            hash: set.hash,
        }
    }
}

fn same_emoji(a: &str, b: &str) -> bool {
    a.chars().filter(|&c| c != '\u{fe0f}').eq(b.chars().filter(|&c| c != '\u{fe0f}'))
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
pub(crate) mod tests {
    use super::StickerSet;
    use crate::enums::StickerType;
    use crate::raw::{base, types};
    use crate::types::StickerSetRef;
    use crate::types::messages_and_media::sticker::tests::{document, sticker_attribute};

    /// Set 10 (access hash 11, hash 77), "cats_by_bot", with stickers 1 and 3 and a non-sticker document 2.
    pub(crate) fn set(masks: bool, emojis: bool, thumb_version: Option<i32>) -> base::messages::StickerSet {
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
            hash: 77,
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
        assert!(cats.stickers.iter().all(|s| s.set_name.as_deref() == Some("cats_by_bot")));
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

    #[test]
    fn keeps_every_emoji_a_pack_tags_a_sticker_with() {
        let base::messages::StickerSet::StickerSet(mut full) = set(false, false, None) else { panic!("no set") };
        let pack = |emoticon: &str, documents: Vec<i64>| {
            base::StickerPack::StickerPack(Box::new(types::StickerPack { emoticon: emoticon.into(), documents }))
        };
        full.packs = vec![pack("😺", vec![1]), pack("❤", vec![1, 3]), pack("😸", vec![3])];
        let cats = StickerSet::parse(base::messages::StickerSet::StickerSet(full)).unwrap();
        assert_eq!(cats.stickers[0].emojis, ["😺", "❤"]);
        assert_eq!(cats.stickers[1].emojis, ["❤", "😸"]);
        let ids = |emoji: &str| cats.stickers_for(emoji).map(|s| s.id).collect::<Vec<_>>();
        assert_eq!((ids("❤️"), ids("😸"), ids("🐶")), (vec![1, 3], vec![3], vec![]));
        assert_eq!((cats.set_ref(), cats.hash), (StickerSetRef::from_id(10, 11), 77));

        // Without packs, a sticker keeps the one emoji of its own alt.
        let plain = StickerSet::parse(set(false, false, None)).unwrap();
        assert_eq!(plain.stickers[1].emojis, ["😸"]);
        assert_eq!(plain.stickers_for("😸").count(), 1);
    }
}
