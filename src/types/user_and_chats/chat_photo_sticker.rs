use crate::enums::ChatPhotoStickerType;
use crate::raw::{base, types};

/// A sticker or custom emoji used as a chat photo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatPhotoSticker {
    /// Whether the photo comes from a custom emoji or a regular sticker.
    pub kind: ChatPhotoStickerType,
    /// Id of the sticker's set. Set only for a regular sticker whose set Telegram names by id.
    pub sticker_set_id: Option<i64>,
    /// Id of the sticker. Set only for a regular sticker.
    pub sticker_id: Option<i64>,
    /// Id of the custom emoji. Set only for a custom emoji.
    pub custom_emoji_id: Option<i64>,
}

impl ChatPhotoSticker {
    /// Read from the first sticker or emoji markup among the photo's videos; `None` when it has neither.
    pub(crate) fn parse(photo: &types::Photo) -> Option<ChatPhotoSticker> {
        photo.video_sizes.iter().flatten().find_map(|size| match size {
            base::VideoSize::VideoSizeEmojiMarkup(m) => Some(ChatPhotoSticker {
                kind: ChatPhotoStickerType::CustomEmoji,
                sticker_set_id: None,
                sticker_id: None,
                custom_emoji_id: Some(m.emoji_id),
            }),
            base::VideoSize::VideoSizeStickerMarkup(m) => Some(ChatPhotoSticker {
                kind: ChatPhotoStickerType::RegularOrMask,
                sticker_set_id: match &m.stickerset {
                    base::InputStickerSet::InputStickerSetID(set) => Some(set.id),
                    _ => None,
                },
                sticker_id: Some(m.sticker_id),
                custom_emoji_id: None,
            }),
            _ => None,
        })
    }
}
