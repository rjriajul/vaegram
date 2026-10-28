use super::send_options::{SendOptions, caption_setters, send_builder};
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::{Message, PeerRef, Upload};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendDocument<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    document: Upload<'a>,
    caption: String,
    parse_mode: Option<ParseMode>,
    options: SendOptions,
}

impl Client {
    pub fn send_document<'a>(
        &'a self,
        chat_id: impl Into<PeerRef>,
        document: impl Into<Upload<'a>>,
    ) -> SendDocument<'a> {
        SendDocument {
            client: self,
            chat_id: chat_id.into(),
            document: document.into(),
            caption: String::new(),
            parse_mode: None,
            options: SendOptions::default(),
        }
    }
}

impl SendDocument<'_> {
    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = caption.into();
        self
    }

    async fn send(self) -> Result<Message> {
        let file = self.client.save_file(self.document).await?;
        let file_name = types::DocumentAttributeFilename { file_name: self.document.name().into() };
        let media = base::InputMedia::InputMediaUploadedDocument(Box::new(types::InputMediaUploadedDocument {
            nosound_video: false,
            force_file: false,
            spoiler: false,
            file,
            thumb: None,
            mime_type: "application/octet-stream".into(),
            attributes: vec![base::DocumentAttribute::DocumentAttributeFilename(Box::new(file_name))],
            stickers: None,
            video_cover: None,
            video_timestamp: None,
            ttl_seconds: None,
        }));
        self.client.send_media(self.chat_id, media, &self.caption, self.parse_mode, self.options).await
    }
}

caption_setters!(SendDocument);
send_builder!(SendDocument);
