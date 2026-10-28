use crate::raw::{base, types};
use crate::types::{Document, Photo};

#[derive(Clone, Debug, PartialEq)]
pub struct FileLocation {
    pub(crate) location: base::InputFileLocation,
    pub dc_id: Option<i32>,
    pub file_size: Option<i64>,
}

// TL types only derive PartialEq because some carry doubles; no file location does, so equality is total.
impl Eq for FileLocation {}

impl From<base::InputFileLocation> for FileLocation {
    fn from(location: base::InputFileLocation) -> Self {
        FileLocation { location, dc_id: None, file_size: None }
    }
}

impl From<&Photo> for FileLocation {
    fn from(photo: &Photo) -> Self {
        photo.location.clone()
    }
}

impl From<&Document> for FileLocation {
    fn from(document: &Document) -> Self {
        document.location.clone()
    }
}

pub(crate) fn input_document(location: &FileLocation) -> base::InputDocument {
    let base::InputFileLocation::InputDocumentFileLocation(l) = &location.location else {
        return base::InputDocument::InputDocumentEmpty;
    };
    let (id, access_hash, file_reference) = (l.id, l.access_hash, l.file_reference.clone());
    base::InputDocument::InputDocument(Box::new(types::InputDocument { id, access_hash, file_reference }))
}

pub(crate) fn input_photo(location: &FileLocation) -> base::InputPhoto {
    let base::InputFileLocation::InputPhotoFileLocation(l) = &location.location else {
        return base::InputPhoto::InputPhotoEmpty;
    };
    let (id, access_hash, file_reference) = (l.id, l.access_hash, l.file_reference.clone());
    base::InputPhoto::InputPhoto(Box::new(types::InputPhoto { id, access_hash, file_reference }))
}
