/// The kind of a sticker, or of the stickers in a set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StickerType {
    /// An ordinary sticker.
    Regular,
    /// A mask placed over faces in photos.
    Mask,
    /// A custom emoji, usable inside message text.
    CustomEmoji,
}
