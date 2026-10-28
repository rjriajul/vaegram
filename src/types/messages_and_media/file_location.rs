use crate::errors::Error;
use crate::file_id::{FileId, FileType, ThumbnailSource};
use crate::raw::{base, types};
use crate::types::{Document, Photo};
use crate::utils::CHANNEL_OFFSET;

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

impl From<&FileId> for FileLocation {
    fn from(id: &FileId) -> Self {
        use ThumbnailSource as S;
        let location = match id.thumbnail_source {
            Some(
                S::ChatPhotoSmall { chat_id, chat_access_hash }
                | S::ChatPhotoBig { chat_id, chat_access_hash }
                | S::ChatPhotoSmallLegacy { chat_id, chat_access_hash }
                | S::ChatPhotoBigLegacy { chat_id, chat_access_hash },
            ) if id.file_type == FileType::ChatPhoto => {
                let peer = if chat_id > 0 {
                    let peer = types::InputPeerUser { user_id: chat_id, access_hash: chat_access_hash };
                    base::InputPeer::InputPeerUser(Box::new(peer))
                } else if chat_id <= -CHANNEL_OFFSET {
                    let channel_id = -chat_id - CHANNEL_OFFSET;
                    let peer = types::InputPeerChannel { channel_id, access_hash: chat_access_hash };
                    base::InputPeer::InputPeerChannel(Box::new(peer))
                } else {
                    base::InputPeer::InputPeerChat(Box::new(types::InputPeerChat { chat_id: -chat_id }))
                };
                let big = matches!(id.thumbnail_source, Some(S::ChatPhotoBig { .. } | S::ChatPhotoBigLegacy { .. }));
                let location = types::InputPeerPhotoFileLocation { big, peer, photo_id: id.media_id };
                base::InputFileLocation::InputPeerPhotoFileLocation(Box::new(location))
            }
            source => {
                let thumb_size = match source {
                    Some(S::Thumbnail { size, .. }) => size.into(),
                    _ => String::new(),
                };
                let (id, access_hash, file_reference, photo) =
                    (id.media_id, id.access_hash, id.file_reference.clone(), id.file_type == FileType::Photo);
                if photo {
                    let location = types::InputPhotoFileLocation { id, access_hash, file_reference, thumb_size };
                    base::InputFileLocation::InputPhotoFileLocation(Box::new(location))
                } else {
                    let location = types::InputDocumentFileLocation { id, access_hash, file_reference, thumb_size };
                    base::InputFileLocation::InputDocumentFileLocation(Box::new(location))
                }
            }
        };
        FileLocation { location, dc_id: Some(id.dc_id), file_size: None }
    }
}

impl TryFrom<&str> for FileLocation {
    type Error = Error;

    fn try_from(file_id: &str) -> Result<Self, Error> {
        Ok(FileLocation::from(&FileId::decode(file_id).map_err(Error::InvalidArgument)?))
    }
}

impl TryFrom<&String> for FileLocation {
    type Error = Error;

    fn try_from(file_id: &String) -> Result<Self, Error> {
        FileLocation::try_from(file_id.as_str())
    }
}

/// The document a download location points at, for the read types that wrap a document.
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

#[cfg(test)]
mod tests {
    use super::FileLocation;
    use crate::file_id::FileId;
    use crate::raw::{Serializable, base, types};
    use crate::types::{Document, Photo};

    const PHOTO: &str = "AgACAgIAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAIG9DEbXX3wSq3oI7t_PqQGAAgBAAMCAAN5AAceBA";
    const DOCUMENT: &str = "BQACAgQAAxkBAAIENGfeY4AfRquwTL2LpDrzqvFMVNt_AAL6C87kooIPtf__________HgQ";

    #[test]
    fn file_ids_download_what_the_media_does() {
        let file_reference = FileId::decode(PHOTO).unwrap().file_reference;
        let size = |t: &str, w, size| types::PhotoSize { r#type: t.into(), w, h: w, size };
        let sizes = [size("m", 320, 9000), size("y", 1280, 90000), size("x", 800, 40000)];
        let photo = base::Photo::Photo(Box::new(types::Photo {
            has_stickers: false,
            id: 5399953792058913798,
            access_hash: 478576178729576621,
            file_reference: file_reference.clone(),
            date: 0,
            sizes: sizes.into_iter().map(|s| base::PhotoSize::PhotoSize(Box::new(s))).collect(),
            video_sizes: None,
            dc_id: 2,
        }));
        let photo = Photo::parse(&photo).unwrap();
        assert_eq!((photo.file_id.as_str(), photo.file_unique_id.as_str()), (PHOTO, "AgADBvQxG1198Eo"));
        let from_id = FileLocation::try_from(PHOTO).unwrap();
        assert_eq!(from_id.location.to_bytes(), photo.location.location.to_bytes());

        let document = base::Document::Document(Box::new(types::Document {
            id: -5399953792058913798,
            access_hash: -1,
            file_reference,
            date: 0,
            mime_type: "application/octet-stream".into(),
            size: 3,
            thumbs: None,
            video_thumbs: None,
            dc_id: 4,
            attributes: Vec::new(),
        }));
        let document = Document::parse(&document).unwrap();
        assert_eq!(document.file_id, DOCUMENT);
        let from_id = FileLocation::try_from(&document.file_id).unwrap();
        assert_eq!((from_id.dc_id, from_id.location), (Some(4), document.location.location));
    }

    #[test]
    fn chat_photo_ids_download_by_peer() {
        let location = FileLocation::try_from("AQADAQADFc1bBwAUAwADLu3E4Rb___9PlyHF_____wAEHgQ").unwrap();
        let peer = types::InputPeerChannel { channel_id: 1234567890, access_hash: -987654321 };
        let peer = base::InputPeer::InputPeerChannel(Box::new(peer));
        let expected = types::InputPeerPhotoFileLocation { big: true, peer, photo_id: 123456789 };
        let expected = base::InputFileLocation::InputPeerPhotoFileLocation(Box::new(expected));
        assert_eq!((location.dc_id, location.location), (Some(1), expected));
        assert!(FileLocation::try_from("not*base64").is_err());
    }
}
