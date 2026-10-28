use crate::raw::types;
use crate::types::{Document, FileLocation};

#[derive(Debug, Clone)]
pub struct Voice {
    pub file_id: String,
    pub file_unique_id: String,
    pub id: i64,
    pub mime_type: String,
    pub file_size: i64,
    pub date: i32,
    pub duration: i32,
    pub waveform: Option<Vec<u8>>,
    pub(crate) location: FileLocation,
}

impl Voice {
    pub(crate) fn parse(document: Document, audio: Option<&types::DocumentAttributeAudio>) -> Voice {
        let Document { file_id, file_unique_id, id, mime_type, file_size, date, location, .. } = document;
        let (duration, waveform) = audio.map_or((0, None), |a| (a.duration, a.waveform.clone()));
        Voice { file_id, file_unique_id, id, mime_type, file_size, date, duration, waveform, location }
    }
}

impl From<&Voice> for FileLocation {
    fn from(voice: &Voice) -> Self {
        voice.location.clone()
    }
}
