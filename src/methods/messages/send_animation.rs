use super::send_options::{SendOptions, send_builder, send_media};
use crate::Client;
use crate::session::transfer::Report;
use crate::types::messages_and_media::media_setters;
use crate::types::{FileSource, InputMedia, PeerRef};

/// Request built by [`Client::send_animation`]. Await it to upload and send.
#[must_use = "a request does nothing until it is awaited"]
pub struct SendAnimation<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    media: InputMedia<'a>,
    options: SendOptions,
    progress: Option<Report<'a>>,
}

impl Client {
    /// Sends a GIF or a silent MPEG-4 video as an animation.
    ///
    /// Awaiting the request uploads the file if needed and returns the sent [`Message`](crate::types::Message).
    pub fn send_animation<'a>(
        &'a self,
        chat_id: impl Into<PeerRef>,
        animation: impl Into<FileSource<'a>>,
    ) -> SendAnimation<'a> {
        let media = InputMedia::animation(animation);
        SendAnimation { client: self, chat_id: chat_id.into(), media, options: SendOptions::default(), progress: None }
    }
}

media_setters!(SendAnimation.media: caption thumb has_spoiler show_caption_above_media duration width height file_name progress);
send_media!(SendAnimation);
