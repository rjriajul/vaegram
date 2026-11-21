/// What a sticker used as a chat photo is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum ChatPhotoStickerType {
    /// A regular or mask sticker, from a sticker set.
    RegularOrMask,
    /// A custom emoji.
    CustomEmoji,
}
