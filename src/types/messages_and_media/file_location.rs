use crate::raw::base;
use crate::types::{Document, Photo};

#[derive(Clone, Debug)]
pub struct FileLocation {
    pub(crate) location: base::InputFileLocation,
    pub dc_id: Option<i32>,
    pub file_size: Option<i64>,
}

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
