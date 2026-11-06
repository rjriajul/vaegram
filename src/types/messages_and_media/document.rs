use crate::file_id::{FileId, FileType, unique_id};
use crate::raw::base;
use crate::types::{FileLocation, Thumbnail};

/// A general file: anything that isn't a video, audio, voice, animation, video note or sticker.
///
/// `==` compares every field, and `file_id` differs between bots and sessions or over time; compare
/// `file_unique_id` to tell whether two values are the same file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// Id for downloading or resending the file. It can change between sessions and accounts.
    pub file_id: String,
    /// Id that stays the same for this file everywhere. It can't be used to download.
    pub file_unique_id: String,
    /// Telegram's document id.
    pub id: i64,
    /// Original file name. `None` if the file has no filename attribute.
    pub file_name: Option<String>,
    /// MIME type of the file, such as `application/pdf`.
    pub mime_type: String,
    /// Size of the file in bytes.
    pub file_size: i64,
    /// Unix time the file was uploaded.
    pub date: i32,
    /// Smaller copies of the file's preview.
    pub thumbs: Vec<Thumbnail>,
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
            thumbs: Thumbnail::from_document(d),
            location: FileLocation { file_size: Some(d.size), ..FileLocation::from(&file_id) },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Document;
    use crate::raw::{base, types};

    #[test]
    fn parses_thumbs() {
        let thumb = types::PhotoSize { r#type: "m".into(), w: 320, h: 180, size: 4_000 };
        let raw = types::Document {
            id: 5,
            access_hash: 6,
            file_reference: vec![3],
            date: 0,
            mime_type: "application/pdf".into(),
            size: 100,
            thumbs: Some(vec![base::PhotoSize::PhotoSize(Box::new(thumb))]),
            video_thumbs: None,
            dc_id: 2,
            attributes: Vec::new(),
        };
        let document = Document::parse(&base::Document::Document(Box::new(raw))).unwrap();
        assert_eq!(document.thumbs.len(), 1);
        assert_eq!((document.thumbs[0].width, document.thumbs[0].file_size), (320, 4_000));
        assert_ne!(document.thumbs[0].file_id, document.file_id);
    }
}
