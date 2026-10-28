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
    pub(crate) fn parse(document: Document, video: Option<&types::DocumentAttributeVideo>) -> VideoNote {
        let Document { id, mime_type, file_size, date, location, .. } = document;
        let (length, duration) = video.map_or((0, 0), |v| (v.w, v.duration as i32));
        VideoNote { id, mime_type, file_size, date, length, duration, location }
    }
}

impl From<&VideoNote> for FileLocation {
    fn from(video_note: &VideoNote) -> Self {
        video_note.location.clone()
    }
}
