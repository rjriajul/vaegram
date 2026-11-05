use crate::types::{FileLocation, MaskPosition, Upload};

/// A sticker to put in a sticker set.
///
/// New files take their format from the extension: `.webp` or `.png` static, `.tgs` animated, `.webm` video.
#[derive(Debug, Clone)]
pub struct InputSticker<'a> {
    pub(crate) file: StickerFile<'a>,
    pub(crate) emoji: String,
    pub(crate) mask_position: Option<MaskPosition>,
    pub(crate) keywords: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) enum StickerFile<'a> {
    Upload(Upload<'a>),
    Stored(FileLocation),
}

impl<'a> InputSticker<'a> {
    /// Creates a sticker from a new file.
    ///
    /// `emoji` holds the one to twenty emoji the sticker stands for, written one after another.
    pub fn new(sticker: impl Into<Upload<'a>>, emoji: impl Into<String>) -> Self {
        InputSticker::with_file(StickerFile::Upload(sticker.into()), emoji.into())
    }

    /// Creates a sticker that's already on Telegram, taken from a [`FileLocation`] instead of uploaded.
    pub fn existing(sticker: impl Into<FileLocation>, emoji: impl Into<String>) -> Self {
        InputSticker::with_file(StickerFile::Stored(sticker.into()), emoji.into())
    }

    fn with_file(file: StickerFile<'a>, emoji: String) -> Self {
        InputSticker { file, emoji, mask_position: None, keywords: None }
    }

    /// Places the sticker on a face, which makes it a mask.
    pub fn mask_position(mut self, position: MaskPosition) -> Self {
        self.mask_position = Some(position);
        self
    }

    /// Adds search keywords for the sticker, joined into one comma-separated string.
    pub fn keywords(mut self, keywords: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keywords = Some(InputSticker::join_keywords(keywords));
        self
    }

    // Telegram keeps a sticker's keywords in one comma-separated string.
    pub(crate) fn join_keywords(keywords: impl IntoIterator<Item = impl Into<String>>) -> String {
        keywords.into_iter().map(Into::into).collect::<Vec<String>>().join(",")
    }
}
