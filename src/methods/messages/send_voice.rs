use super::send_options::{SendOptions, send_builder, send_media};
use crate::Client;
use crate::session::transfer::Report;
use crate::types::messages_and_media::media_setters;
use crate::types::{FileSource, InputMedia, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendVoice<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    media: InputMedia<'a>,
    options: SendOptions,
    progress: Option<Report<'a>>,
}

impl Client {
    pub fn send_voice<'a>(&'a self, chat_id: impl Into<PeerRef>, voice: impl Into<FileSource<'a>>) -> SendVoice<'a> {
        let media = InputMedia::voice(voice);
        SendVoice { client: self, chat_id: chat_id.into(), media, options: SendOptions::default(), progress: None }
    }
}

media_setters!(SendVoice.media: caption duration file_name progress);
send_media!(SendVoice);
