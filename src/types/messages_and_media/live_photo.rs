use crate::file_id::FileType;
use crate::raw::{base, types};
use crate::types::{Document, FileLocation, Photo};

/// A photo with a short video that plays when it is held. The ids, sizes and download are the video's.
///
/// `==` compares every field, and `file_id` differs between bots and sessions or over time; compare
/// `file_unique_id` to tell whether two values are the same file.
#[derive(Debug, Clone, PartialEq)]
pub struct LivePhoto {
    /// The still photo. `None` if Telegram sent none or it can't be downloaded.
    pub photo: Option<Photo>,
    /// Identifier of the video file, for downloading it.
    pub file_id: String,
    /// Identifier of the video file that stays the same across clients and sessions.
    pub file_unique_id: String,
    /// Telegram's id of the video document.
    pub id: i64,
    /// MIME type of the video.
    pub mime_type: String,
    /// Size of the video in bytes.
    pub file_size: i64,
    /// Width of the video, in pixels. `0` if Telegram sent no video attributes.
    pub width: i32,
    /// Height of the video, in pixels. `0` if Telegram sent no video attributes.
    pub height: i32,
    /// Duration of the video, in seconds. `0` if Telegram sent no video attributes.
    pub duration: i32,
    pub(crate) location: FileLocation,
}

impl LivePhoto {
    /// The video makes it a live photo; without a readable one it is a plain photo, for filters as well.
    pub(crate) fn is_live(media: &types::MessageMediaPhoto) -> bool {
        media.live_photo && matches!(media.video, Some(base::Document::Document(_)))
    }

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
