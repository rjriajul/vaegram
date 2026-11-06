use crate::raw::types;
use crate::types::{Document, FileLocation};

/// A voice message.
///
/// `==` compares every field, and `file_id` differs between bots and sessions or over time; compare
/// `file_unique_id` to tell whether two values are the same file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Voice {
    /// Id for downloading or resending the file. It can change between sessions and accounts.
    pub file_id: String,
    /// Id that stays the same for this file everywhere. It can't be used to download.
    pub file_unique_id: String,
    /// Telegram's document id.
    pub id: i64,
    /// MIME type of the file, such as `audio/ogg`.
    pub mime_type: String,
    /// Size of the file in bytes.
    pub file_size: i64,
    /// Unix time the file was uploaded.
    pub date: i32,
    /// Length in seconds. `0` if Telegram sent no audio attributes.
    pub duration: i32,
    /// Raw waveform bytes as Telegram sends them. `None` if Telegram sends no waveform.
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
