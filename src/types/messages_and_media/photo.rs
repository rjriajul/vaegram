use crate::file_id::{FileId, FileType, ThumbnailSource, unique_id};
use crate::raw::base;
use crate::types::{FileLocation, Thumbnail};

use super::thumbnail::full_size;

/// A photo, represented by its largest size.
#[derive(Clone, Debug, PartialEq)]
pub struct Photo {
    /// File id to download or resend the photo with.
    pub file_id: String,
    /// Stable for this photo here, but not equal to the Bot API's `file_unique_id` for it.
    pub file_unique_id: String,
    /// Telegram's id of the photo.
    pub id: i64,
    /// Width of the largest size, in pixels.
    pub width: i32,
    /// Height of the largest size, in pixels.
    pub height: i32,
    /// File size of the largest size, in bytes.
    pub file_size: i64,
    /// Unix time the photo was uploaded.
    pub date: i32,
    /// Smaller sizes of the photo.
    pub thumbs: Vec<Thumbnail>,
    pub(crate) location: FileLocation,
}

impl Photo {
    /// Whether `parse` gives a photo: one with a size to download, which a stripped or cached-only photo lacks.
    pub(crate) fn is_downloadable(photo: &base::Photo) -> bool {
        let base::Photo::Photo(p) = photo else { return false };
        full_size(&p.sizes).is_some_and(|size| !size.0.is_empty())
    }

    pub(crate) fn parse(photo: &base::Photo) -> Option<Photo> {
        let base::Photo::Photo(p) = photo else { return None };
        let (thumb_size, width, height, file_size) = full_size(&p.sizes)?;
        let source = ThumbnailSource::Thumbnail { file_type: FileType::Photo, size: thumb_size.chars().next()? };
        let file_id = FileId {
            thumbnail_source: Some(source),
            ..FileId::new(FileType::Photo, p.dc_id, p.id, p.access_hash, &p.file_reference)
        };
        Some(Photo {
            file_id: file_id.encode(),
            file_unique_id: unique_id(p.id),
            id: p.id,
            width,
            height,
            file_size,
            date: p.date,
            thumbs: Thumbnail::from_photo(p),
            location: FileLocation { file_size: Some(file_size), ..FileLocation::from(&file_id) },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::Photo;
    use crate::raw::{base, types};

    #[test]
    fn parses_thumbs() {
        let size = |letter: &str, w, h, bytes| {
            base::PhotoSize::PhotoSize(Box::new(types::PhotoSize { r#type: letter.into(), w, h, size: bytes }))
        };
        let raw = types::Photo {
            has_stickers: false,
            id: 7,
            access_hash: 8,
            file_reference: vec![9],
            date: 0,
            sizes: vec![size("s", 90, 60, 1_000), size("x", 1280, 853, 90_000)],
            video_sizes: None,
            dc_id: 2,
        };
        let photo = Photo::parse(&base::Photo::Photo(Box::new(raw))).unwrap();
        assert_eq!((photo.width, photo.height), (1280, 853));
        assert_eq!(photo.thumbs.len(), 1);
        assert_eq!((photo.thumbs[0].width, photo.thumbs[0].height), (90, 60));
        assert_ne!(photo.thumbs[0].file_unique_id, photo.file_unique_id);
    }
}
