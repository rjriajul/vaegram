use crate::raw::types;
use crate::types::{Document, FileLocation, Thumbnail};

#[derive(Debug, Clone)]
pub struct VideoNote {
    pub file_id: String,
    pub file_unique_id: String,
    pub id: i64,
    pub mime_type: String,
    pub file_size: i64,
    pub date: i32,
    pub length: i32,
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
