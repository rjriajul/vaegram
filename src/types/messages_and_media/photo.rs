use crate::file_id::{FileId, FileType, ThumbnailSource, unique_id};
use crate::raw::base;
use crate::types::FileLocation;

#[derive(Clone, Debug, PartialEq)]
pub struct Photo {
    pub file_id: String,
    /// Stable for this photo here, but not equal to the Bot API's `file_unique_id` for it.
    pub file_unique_id: String,
    pub id: i64,
    pub width: i32,
    pub height: i32,
    pub file_size: i64,
    pub date: i32,
    pub(crate) location: FileLocation,
}

impl Photo {
    pub(crate) fn parse(photo: &base::Photo) -> Option<Photo> {
        let base::Photo::Photo(p) = photo else { return None };
        let (thumb_size, width, height, file_size) = p
            .sizes
            .iter()
            .filter_map(|s| match s {
                base::PhotoSize::PhotoSize(s) => Some((&s.r#type, s.w, s.h, s.size)),
                base::PhotoSize::PhotoSizeProgressive(s) => Some((&s.r#type, s.w, s.h, *s.sizes.iter().max()?)),
                _ => None,
            })
            .max_by_key(|s| s.3)?;
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
            file_size: file_size.into(),
            date: p.date,
            location: FileLocation { file_size: Some(file_size.into()), ..FileLocation::from(&file_id) },
        })
    }
}
