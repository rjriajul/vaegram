use super::send_options::{SendOptions, send_builder, send_media};
use crate::Client;
use crate::session::transfer::Report;
use crate::types::messages_and_media::media_setters;
use crate::types::{FileSource, InputMedia, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendSticker<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    media: InputMedia<'a>,
    options: SendOptions,
    progress: Option<Report<'a>>,
}

impl Client {
    pub fn send_sticker<'a>(
        &'a self,
        chat_id: impl Into<PeerRef>,
        sticker: impl Into<FileSource<'a>>,
    ) -> SendSticker<'a> {
        let media = InputMedia::sticker(sticker);
        SendSticker { client: self, chat_id: chat_id.into(), media, options: SendOptions::default(), progress: None }
    }
}

media_setters!(SendSticker.media: emoji progress);
send_media!(SendSticker);
