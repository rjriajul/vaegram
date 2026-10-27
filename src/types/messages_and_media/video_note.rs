use crate::raw::types;
use crate::types::{Document, FileLocation};

#[derive(Debug, Clone)]
pub struct VideoNote {
    pub id: i64,
    pub mime_type: String,
    pub file_size: i64,
    pub date: i32,
    pub length: i32,
    pub duration: i32,
    pub(crate) location: FileLocation,
}

impl VideoNote {
    pub(crate) fn parse(document: Document, video: &types::DocumentAttributeVideo) -> VideoNote {
        let Document { id, mime_type, file_size, date, location, .. } = document;
        VideoNote { id, mime_type, file_size, date, length: video.w, duration: video.duration as i32, location }
    }
}

impl From<&VideoNote> for FileLocation {
    fn from(video_note: &VideoNote) -> Self {
        video_note.location.clone()
    }
}
