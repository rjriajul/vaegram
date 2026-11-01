use super::send_options::{SendOptions, send_builder, send_media};
use crate::Client;
use crate::session::transfer::Report;
use crate::types::messages_and_media::media_setters;
use crate::types::{FileSource, InputMedia, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendVideo<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    media: InputMedia<'a>,
    options: SendOptions,
    progress: Option<Report<'a>>,
}

impl Client {
    pub fn send_video<'a>(&'a self, chat_id: impl Into<PeerRef>, video: impl Into<FileSource<'a>>) -> SendVideo<'a> {
        let media = InputMedia::video(video);
        SendVideo { client: self, chat_id: chat_id.into(), media, options: SendOptions::default(), progress: None }
    }
}

media_setters!(SendVideo.media: caption thumb cover has_spoiler show_caption_above_media supports_streaming duration start_timestamp width height file_name progress);
send_media!(SendVideo);
