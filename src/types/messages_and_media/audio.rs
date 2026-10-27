use crate::raw::types;
use crate::types::{Document, FileLocation};

#[derive(Debug, Clone)]
pub struct Audio {
    pub id: i64,
    pub file_name: Option<String>,
    pub mime_type: String,
    pub file_size: i64,
    pub date: i32,
    pub duration: i32,
    pub performer: Option<String>,
    pub title: Option<String>,
    pub(crate) location: FileLocation,
}

impl Audio {
    pub(crate) fn parse(document: Document, audio: &types::DocumentAttributeAudio) -> Audio {
        let Document { id, file_name, mime_type, file_size, date, location } = document;
        Audio {
            id,
            file_name,
            mime_type,
            file_size,
            date,
            duration: audio.duration,
            performer: audio.performer.clone(),
            title: audio.title.clone(),
            location,
        }
    }
}

impl From<&Audio> for FileLocation {
    fn from(audio: &Audio) -> Self {
        audio.location.clone()
    }
}
