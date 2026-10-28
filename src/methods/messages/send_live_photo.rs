use super::send_options::{SendOptions, send_builder, send_media};
use crate::Client;
use crate::session::transfer::Report;
use crate::types::messages_and_media::media_setters;
use crate::types::{FileSource, InputMedia, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendLivePhoto<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    media: InputMedia<'a>,
    options: SendOptions,
    progress: Option<Report<'a>>,
}

impl Client {
    /// Sends a photo that plays `live_photo`, a video of up to 10 seconds and 10 MB, when held.
    pub fn send_live_photo<'a>(
        &'a self,
        chat_id: impl Into<PeerRef>,
        photo: impl Into<FileSource<'a>>,
        live_photo: impl Into<FileSource<'a>>,
    ) -> SendLivePhoto<'a> {
        SendLivePhoto {
            client: self,
            chat_id: chat_id.into(),
            media: InputMedia::live_photo(photo, live_photo),
            options: SendOptions::default(),
            progress: None,
        }
    }
}

media_setters!(SendLivePhoto.media: caption has_spoiler show_caption_above_media progress);
send_media!(SendLivePhoto);
