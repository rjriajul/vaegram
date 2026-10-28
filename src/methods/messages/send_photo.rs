use super::send_options::{SendOptions, send_builder, send_media};
use crate::Client;
use crate::session::transfer::Report;
use crate::types::messages_and_media::media_setters;
use crate::types::{FileSource, InputMedia, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendPhoto<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    media: InputMedia<'a>,
    options: SendOptions,
    progress: Option<Report<'a>>,
}

impl Client {
    pub fn send_photo<'a>(&'a self, chat_id: impl Into<PeerRef>, photo: impl Into<FileSource<'a>>) -> SendPhoto<'a> {
        let media = InputMedia::photo(photo);
        SendPhoto { client: self, chat_id: chat_id.into(), media, options: SendOptions::default(), progress: None }
    }
}

media_setters!(SendPhoto.media: caption has_spoiler show_caption_above_media progress);
send_media!(SendPhoto);
