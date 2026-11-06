use crate::raw::types;
use crate::types::{Document, FileLocation, Thumbnail};

/// A video file.
#[derive(Debug, Clone, PartialEq)]
pub struct Video {
    /// Id for downloading or resending the file. It can change between sessions and accounts.
    pub file_id: String,
    /// Id that stays the same for this file everywhere. It can't be used to download.
    pub file_unique_id: String,
    /// Telegram's document id.
    pub id: i64,
    /// Original file name. `None` if the sender's client didn't set one.
    pub file_name: Option<String>,
    /// MIME type of the file, such as `video/mp4`.
    pub mime_type: String,
    /// Size of the file in bytes.
    pub file_size: i64,
    /// Unix time the file was uploaded.
    pub date: i32,
    /// Width in pixels. `0` if Telegram sent no video attributes.
    pub width: i32,
    /// Height in pixels. `0` if Telegram sent no video attributes.
    pub height: i32,
    /// Length in seconds. `0` if Telegram sent no video attributes.
    pub duration: i32,
    /// `true` if the video can be played while it downloads.
    pub supports_streaming: bool,
    /// Smaller copies of the file's preview.
    pub thumbs: Vec<Thumbnail>,
    pub(crate) location: FileLocation,
}

impl Video {
    pub(crate) fn parse(document: Document, video: Option<&types::DocumentAttributeVideo>) -> Video {
        let Document { file_id, file_unique_id, id, file_name, mime_type, file_size, date, thumbs, location } =
            document;
        Video {
            file_id,
            file_unique_id,
            id,
            file_name,
            mime_type,
            file_size,
            date,
            width: video.map_or(0, |v| v.w),
            height: video.map_or(0, |v| v.h),
            duration: video.map_or(0, |v| v.duration as i32),
            supports_streaming: video.is_some_and(|v| v.supports_streaming),
            thumbs,
            location,
        }
    }
}

impl From<&Video> for FileLocation {
    fn from(video: &Video) -> Self {
        video.location.clone()
    }
}
