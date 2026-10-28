use crate::raw::types;
use crate::types::{Document, FileLocation};

#[derive(Debug, Clone)]
pub struct Video {
    pub file_id: String,
    pub file_unique_id: String,
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
    pub(crate) fn parse(document: Document, video: Option<&types::DocumentAttributeVideo>) -> Video {
        let Document { file_id, file_unique_id, id, file_name, mime_type, file_size, date, location } = document;
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
            location,
        }
    }
}

impl From<&Video> for FileLocation {
    fn from(video: &Video) -> Self {
        video.location.clone()
    }
}
