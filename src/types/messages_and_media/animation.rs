use crate::raw::types;
use crate::types::{Document, FileLocation};

#[derive(Debug, Clone)]
pub struct Animation {
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
    pub(crate) location: FileLocation,
}

impl Animation {
    /// `video` is absent for real GIFs, which only carry an image size.
    pub(crate) fn parse(
        document: Document,
        video: Option<&types::DocumentAttributeVideo>,
        size: Option<(i32, i32)>,
    ) -> Animation {
        let Document { file_id, file_unique_id, id, file_name, mime_type, file_size, date, location } = document;
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
            location,
        }
    }
}

impl From<&Animation> for FileLocation {
    fn from(animation: &Animation) -> Self {
        animation.location.clone()
    }
}
