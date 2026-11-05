use crate::file_id::{FileId, FileType, ThumbnailSource, thumbnail_unique_id};
use crate::raw::{base, types};

/// The video of an animated chat photo. Pass `file_id` to `download_media` to fetch it.
#[derive(Debug, Clone)]
pub struct AnimatedChatPhoto {
    /// The width and height of the square video.
    pub length: i32,
    /// File id that `download_media` accepts.
    pub file_id: String,
    /// Id that stays the same across sessions and clients. It can't be used to download the video.
    pub file_unique_id: String,
    /// Size of the video in bytes.
    pub file_size: i64,
    /// Second of the video used as the still photo. `None` if Telegram doesn't set one.
    pub main_frame_timestamp: Option<f64>,
}

// The timestamp is compared by its bits, so equality is total and chats can stay `Eq`.
impl PartialEq for AnimatedChatPhoto {
    fn eq(&self, other: &AnimatedChatPhoto) -> bool {
        self.length == other.length
            && self.file_id == other.file_id
            && self.file_unique_id == other.file_unique_id
            && self.file_size == other.file_size
            && self.main_frame_timestamp.map(f64::to_bits) == other.main_frame_timestamp.map(f64::to_bits)
    }
}

impl Eq for AnimatedChatPhoto {}

impl AnimatedChatPhoto {
    /// Built from the largest video of the full photo. `None` when it has none.
    pub(crate) fn parse(photo: &types::Photo) -> Option<AnimatedChatPhoto> {
        let video = photo
            .video_sizes
            .iter()
            .flatten()
            .filter_map(|size| match size {
                base::VideoSize::VideoSize(v) => Some(v),
                _ => None,
            })
            .max_by_key(|v| i64::from(v.w) * i64::from(v.h))?;
        let size = video.r#type.chars().next()?;
        let source = ThumbnailSource::Thumbnail { file_type: FileType::Photo, size };
        let file_id = FileId {
            thumbnail_source: Some(source),
            ..FileId::new(FileType::Photo, photo.dc_id, photo.id, photo.access_hash, &photo.file_reference)
        };
        Some(AnimatedChatPhoto {
            length: video.w,
            file_id: file_id.encode(),
            file_unique_id: thumbnail_unique_id(photo.id, size),
            file_size: video.size.into(),
            main_frame_timestamp: video.video_start_ts,
        })
    }
}
