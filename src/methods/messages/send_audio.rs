use super::send_options::{SendOptions, send_builder, send_media};
use crate::Client;
use crate::session::transfer::Report;
use crate::types::messages_and_media::media_setters;
use crate::types::{FileSource, InputMedia, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendAudio<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    media: InputMedia<'a>,
    options: SendOptions,
    progress: Option<Report<'a>>,
}

impl Client {
    pub fn send_audio<'a>(&'a self, chat_id: impl Into<PeerRef>, audio: impl Into<FileSource<'a>>) -> SendAudio<'a> {
        let media = InputMedia::audio(audio);
        SendAudio { client: self, chat_id: chat_id.into(), media, options: SendOptions::default(), progress: None }
    }
}

media_setters!(SendAudio.media: caption thumb duration performer title progress);
send_media!(SendAudio);
