use crate::raw::types;
use crate::types::{Document, FileLocation, Thumbnail};

/// A GIF or silent MPEG-4 clip that clients play in a loop.
#[derive(Debug, Clone)]
pub struct Animation {
    /// Encoded identifier for sending or downloading the file again.
    pub file_id: String,
    /// Id that stays the same for this file everywhere. It can't be used to download.
    pub file_unique_id: String,
    /// Telegram's id of the document.
    pub id: i64,
    /// Original file name. `None` if the file has no filename attribute.
    pub file_name: Option<String>,
    /// MIME type reported by Telegram.
    pub mime_type: String,
    /// Size in bytes.
    pub file_size: i64,
    /// When the file was uploaded, as a Unix timestamp.
    pub date: i32,
    /// Width in pixels. 0 if Telegram sent no size.
    pub width: i32,
    /// Height in pixels. 0 if Telegram sent no size.
    pub height: i32,
    /// Length in seconds. 0 if Telegram sent no video attribute.
    pub duration: i32,
    /// Smaller copies of the file's preview.
    pub thumbs: Vec<Thumbnail>,
    pub(crate) location: FileLocation,
}

impl Animation {
    /// `video` is absent for real GIFs, which only carry an image size.
    pub(crate) fn parse(
        document: Document,
        video: Option<&types::DocumentAttributeVideo>,
        size: Option<(i32, i32)>,
    ) -> Animation {
        let Document { file_id, file_unique_id, id, file_name, mime_type, file_size, date, thumbs, location } =
            document;
        let (width, height) = video.map(|v| (v.w, v.h)).or(size).unwrap_or_default();
        let duration = video.map_or(0, |v| v.duration as i32);
        Animation {
            file_id,
            file_unique_id,
            id,
            file_name,
            mime_type,
            file_size,
            date,
            width,
            height,
            duration,
            thumbs,
            location,
        }
    }
}

impl From<&Animation> for FileLocation {
    fn from(animation: &Animation) -> Self {
        animation.location.clone()
    }
}
