use crate::file_id::{FileId, FileType, unique_id};
use crate::raw::base;
use crate::types::FileLocation;

/// A general file: anything that isn't a video, audio, voice, animation, video note or sticker.
#[derive(Clone, Debug)]
pub struct Document {
    pub file_id: String,
    pub file_unique_id: String,
    pub id: i64,
    pub file_name: Option<String>,
    pub mime_type: String,
    pub file_size: i64,
    pub date: i32,
    pub(crate) location: FileLocation,
}

impl Document {
    pub(crate) fn parse(document: &base::Document) -> Option<Document> {
        Document::parse_as(document, FileType::Document)
    }

    pub(crate) fn parse_as(document: &base::Document, file_type: FileType) -> Option<Document> {
        let base::Document::Document(d) = document else { return None };
        let file_name = d.attributes.iter().find_map(|a| match a {
            base::DocumentAttribute::DocumentAttributeFilename(f) => Some(f.file_name.clone()),
            _ => None,
        });
        let file_id = FileId::new(file_type, d.dc_id, d.id, d.access_hash, &d.file_reference);
        Some(Document {
            file_id: file_id.encode(),
            file_unique_id: unique_id(d.id),
            id: d.id,
            file_name,
            mime_type: d.mime_type.clone(),
            file_size: d.size,
            date: d.date,
            location: FileLocation { file_size: Some(d.size), ..FileLocation::from(&file_id) },
        })
    }
}
