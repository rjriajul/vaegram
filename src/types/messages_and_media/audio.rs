use crate::raw::types;
use crate::types::{Document, FileLocation, Thumbnail};

/// An audio file shown as music, as opposed to a voice note.
#[derive(Debug, Clone)]
pub struct Audio {
    /// Encoded identifier for sending or downloading the file again.
    pub file_id: String,
    /// Id that stays the same for this file everywhere. It can't be used to download.
    pub file_unique_id: String,
    /// Telegram's id of the document.
    pub id: i64,
    /// Original file name. `None` if the file has no filename attribute.
    pub file_name: Option<String>,
    /// MIME type reported by Telegram.
    pub mime_type: String,
    /// Size in bytes.
    pub file_size: i64,
    /// When the file was uploaded, as a Unix timestamp.
    pub date: i32,
    /// Length in seconds. 0 if Telegram sent no audio attribute.
    pub duration: i32,
    /// Artist name from the file's tags. `None` if absent.
    pub performer: Option<String>,
    /// Track title from the file's tags. `None` if absent.
    pub title: Option<String>,
    /// Smaller copies of the file's preview.
    pub thumbs: Vec<Thumbnail>,
    pub(crate) location: FileLocation,
}

impl Audio {
    pub(crate) fn parse(document: Document, audio: Option<&types::DocumentAttributeAudio>) -> Audio {
        let Document { file_id, file_unique_id, id, file_name, mime_type, file_size, date, thumbs, location } =
            document;
        Audio {
            file_id,
            file_unique_id,
            id,
            file_name,
            mime_type,
            file_size,
            date,
            duration: audio.map_or(0, |a| a.duration),
            performer: audio.and_then(|a| a.performer.clone()),
            title: audio.and_then(|a| a.title.clone()),
            thumbs,
            location,
        }
    }
}

impl From<&Audio> for FileLocation {
    fn from(audio: &Audio) -> Self {
        audio.location.clone()
    }
}
