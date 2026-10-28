use crate::file_id::FileType;
use crate::raw::{base, types};
use crate::types::{Document, FileLocation, Photo};

/// A photo with a short video that plays when it is held. The ids, sizes and download are the video's.
#[derive(Debug, Clone)]
pub struct LivePhoto {
    pub photo: Option<Photo>,
    pub file_id: String,
    pub file_unique_id: String,
    pub id: i64,
    pub mime_type: String,
    pub file_size: i64,
    pub width: i32,
    pub height: i32,
    pub duration: i32,
    pub(crate) location: FileLocation,
}

impl LivePhoto {
    pub(crate) fn parse(media: &types::MessageMediaPhoto) -> Option<LivePhoto> {
        let video = media.video.as_ref()?;
        let base::Document::Document(raw) = video else { return None };
        let (width, height, duration) = raw
            .attributes
            .iter()
            .find_map(|a| match a {
                base::DocumentAttribute::DocumentAttributeVideo(v) => Some((v.w, v.h, v.duration as i32)),
                _ => None,
            })
            .unwrap_or_default();
        let Document { file_id, file_unique_id, id, mime_type, file_size, location, .. } =
            Document::parse_as(video, FileType::Video)?;
        Some(LivePhoto {
            photo: media.photo.as_ref().and_then(Photo::parse),
            file_id,
            file_unique_id,
            id,
            mime_type,
            file_size,
            width,
            height,
            duration,
            location,
        })
    }
}

impl From<&LivePhoto> for FileLocation {
    fn from(live_photo: &LivePhoto) -> Self {
        live_photo.location.clone()
    }
}
