use crate::raw::types;
use crate::types::{Document, FileLocation};

#[derive(Debug, Clone)]
pub struct Video {
    pub id: i64,
    pub file_name: Option<String>,
    pub mime_type: String,
    pub file_size: i64,
    pub date: i32,
    pub width: i32,
    pub height: i32,
    pub duration: i32,
    pub supports_streaming: bool,
    pub(crate) location: FileLocation,
}

impl Video {
    pub(crate) fn parse(document: Document, video: &types::DocumentAttributeVideo) -> Video {
        let Document { id, file_name, mime_type, file_size, date, location } = document;
        Video {
            id,
            file_name,
            mime_type,
            file_size,
            date,
            width: video.w,
            height: video.h,
            duration: video.duration as i32,
            supports_streaming: video.supports_streaming,
            location,
        }
    }
}

impl From<&Video> for FileLocation {
    fn from(video: &Video) -> Self {
        video.location.clone()
    }
}
