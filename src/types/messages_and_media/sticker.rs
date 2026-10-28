use crate::raw::types;
use crate::types::{Document, FileLocation};

#[derive(Debug, Clone)]
pub struct Sticker {
    pub id: i64,
    pub mime_type: String,
    pub file_size: i64,
    pub date: i32,
    pub width: i32,
    pub height: i32,
    pub is_animated: bool,
    pub is_video: bool,
    pub emoji: Option<String>,
    pub(crate) location: FileLocation,
}

impl Sticker {
    pub(crate) fn parse(
        document: Document,
        sticker: &types::DocumentAttributeSticker,
        video: Option<&types::DocumentAttributeVideo>,
        size: Option<(i32, i32)>,
    ) -> Sticker {
        let Document { id, mime_type, file_size, date, location, .. } = document;
        let (width, height) = video.map(|v| (v.w, v.h)).or(size).unwrap_or_default();
        Sticker {
            id,
            is_animated: mime_type == "application/x-tgsticker",
            is_video: mime_type == "video/webm",
            mime_type,
            file_size,
            date,
            width,
            height,
            emoji: (!sticker.alt.is_empty()).then(|| sticker.alt.clone()),
            location,
        }
    }
}

impl From<&Sticker> for FileLocation {
    fn from(sticker: &Sticker) -> Self {
        sticker.location.clone()
    }
}
