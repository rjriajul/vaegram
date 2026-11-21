use crate::enums::FileType;
use crate::raw::base;
use crate::types::{Document, Photo};

/// A named album that groups stories on a profile.
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct StoryAlbum {
    /// Id of the album.
    pub id: i32,
    /// Title of the album.
    pub title: String,
    /// Photo shown as the album icon. `None` if it has none or the icon isn't a photo.
    pub icon_photo: Option<Photo>,
    /// Video shown as the album icon. `None` if it has none or the icon isn't a video.
    pub icon_video: Option<Document>,
}

impl StoryAlbum {
    pub(crate) fn parse(raw: base::StoryAlbum) -> StoryAlbum {
        let base::StoryAlbum::StoryAlbum(album) = raw;
        let album = *album;
        StoryAlbum {
            id: album.album_id,
            title: album.title,
            icon_photo: album.icon_photo.as_ref().and_then(Photo::parse),
            icon_video: album.icon_video.as_ref().and_then(|video| Document::parse_as(video, FileType::Video)),
        }
    }
}
