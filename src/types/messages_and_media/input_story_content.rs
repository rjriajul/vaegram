use crate::Client;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::Upload;

/// What a story shows: a 1080×1920 photo, or a 720×1280 MPEG4 video of up to 60 seconds.
#[derive(Clone, Copy, Debug)]
pub enum InputStoryContent<'a> {
    Photo(Upload<'a>),
    /// Times are in seconds; an animation plays without sound.
    Video {
        video: Upload<'a>,
        duration: f64,
        cover_frame_timestamp: f64,
        is_animation: bool,
    },
}

impl InputStoryContent<'_> {
    pub(crate) async fn upload(self, client: &Client) -> Result<base::InputMedia> {
        Ok(match self {
            InputStoryContent::Photo(photo) => {
                base::InputMedia::InputMediaUploadedPhoto(Box::new(types::InputMediaUploadedPhoto {
                    spoiler: false,
                    live_photo: false,
                    file: client.save_file(photo).await?,
                    stickers: None,
                    ttl_seconds: None,
                    video: None,
                }))
            }
            InputStoryContent::Video { video, duration, cover_frame_timestamp, is_animation } => {
                let attribute = types::DocumentAttributeVideo {
                    round_message: false,
                    supports_streaming: true,
                    nosound: is_animation,
                    duration,
                    w: 720,
                    h: 1280,
                    preload_prefix_size: None,
                    video_start_ts: Some(cover_frame_timestamp),
                    video_codec: None,
                };
                let file_name = types::DocumentAttributeFilename { file_name: video.name().into() };
                base::InputMedia::InputMediaUploadedDocument(Box::new(types::InputMediaUploadedDocument {
                    nosound_video: true,
                    force_file: false,
                    spoiler: false,
                    file: client.save_file(video).await?,
                    thumb: None,
                    mime_type: "video/mp4".into(),
                    attributes: vec![
                        base::DocumentAttribute::DocumentAttributeVideo(Box::new(attribute)),
                        base::DocumentAttribute::DocumentAttributeFilename(Box::new(file_name)),
                    ],
                    stickers: None,
                    video_cover: None,
                    video_timestamp: None,
                    ttl_seconds: None,
                }))
            }
        })
    }
}
