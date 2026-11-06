use crate::raw::types;
use crate::types::{Document, FileLocation, Thumbnail};

/// A round video message.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoNote {
    /// Id for downloading or resending the file. It can change between sessions and accounts.
    pub file_id: String,
    /// Id that stays the same for this file everywhere. It can't be used to download.
    pub file_unique_id: String,
    /// Telegram's document id.
    pub id: i64,
    /// MIME type of the file, such as `video/mp4`.
    pub mime_type: String,
    /// Size of the file in bytes.
    pub file_size: i64,
    /// Unix time the file was uploaded.
    pub date: i32,
    /// Diameter of the circle in pixels. `0` if Telegram sent no video attributes.
    pub length: i32,
    /// Length in seconds. `0` if Telegram sent no video attributes.
    pub duration: i32,
    /// Smaller copies of the file's preview.
    pub thumbs: Vec<Thumbnail>,
    pub(crate) location: FileLocation,
}

impl VideoNote {
    pub(crate) fn parse(document: Document, video: Option<&types::DocumentAttributeVideo>) -> VideoNote {
        let Document { file_id, file_unique_id, id, mime_type, file_size, date, thumbs, location, .. } = document;
        let (length, duration) = video.map_or((0, 0), |v| (v.w, v.duration as i32));
        VideoNote { file_id, file_unique_id, id, mime_type, file_size, date, length, duration, thumbs, location }
    }
}

impl From<&VideoNote> for FileLocation {
    fn from(video_note: &VideoNote) -> Self {
        video_note.location.clone()
    }
}
