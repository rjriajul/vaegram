use crate::raw::{base, types};
use crate::types::FileLocation;

/// A general file: anything that isn't a video, audio, voice, animation, video note or sticker.
#[derive(Clone, Debug)]
pub struct Document {
    pub id: i64,
    pub file_name: Option<String>,
    pub mime_type: String,
    pub file_size: i64,
    pub date: i32,
    pub(crate) location: FileLocation,
}

impl Document {
    pub(crate) fn parse(document: &base::Document) -> Option<Document> {
        let base::Document::Document(d) = document else { return None };
        let file_name = d.attributes.iter().find_map(|a| match a {
            base::DocumentAttribute::DocumentAttributeFilename(f) => Some(f.file_name.clone()),
            _ => None,
        });
        let location = types::InputDocumentFileLocation {
            id: d.id,
            access_hash: d.access_hash,
            file_reference: d.file_reference.clone(),
            thumb_size: String::new(),
        };
        Some(Document {
            id: d.id,
            file_name,
            mime_type: d.mime_type.clone(),
            file_size: d.size,
            date: d.date,
            location: FileLocation {
                location: base::InputFileLocation::InputDocumentFileLocation(Box::new(location)),
                dc_id: Some(d.dc_id),
                file_size: Some(d.size),
            },
        })
    }
}
