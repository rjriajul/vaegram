use crate::raw::{base, types};
use crate::types::FileLocation;

#[derive(Clone, Debug, PartialEq)]
pub struct Photo {
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
        let location = types::InputPhotoFileLocation {
            id: p.id,
            access_hash: p.access_hash,
            file_reference: p.file_reference.clone(),
            thumb_size: thumb_size.clone(),
        };
        Some(Photo {
            id: p.id,
            width,
            height,
            file_size: file_size.into(),
            date: p.date,
            location: FileLocation {
                location: base::InputFileLocation::InputPhotoFileLocation(Box::new(location)),
                dc_id: Some(p.dc_id),
                file_size: Some(file_size.into()),
            },
        })
    }
}
