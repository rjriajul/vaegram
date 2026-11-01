use super::send_options::{SendOptions, send_builder, send_media};
use crate::Client;
use crate::session::transfer::Report;
use crate::types::messages_and_media::media_setters;
use crate::types::{FileSource, InputMedia, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendDocument<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    media: InputMedia<'a>,
    options: SendOptions,
    progress: Option<Report<'a>>,
}

impl Client {
    pub fn send_document<'a>(
        &'a self,
        chat_id: impl Into<PeerRef>,
        document: impl Into<FileSource<'a>>,
    ) -> SendDocument<'a> {
        let media = InputMedia::document(document);
        SendDocument { client: self, chat_id: chat_id.into(), media, options: SendOptions::default(), progress: None }
    }
}

media_setters!(SendDocument.media: caption thumb disable_content_type_detection file_name progress);
send_media!(SendDocument);
