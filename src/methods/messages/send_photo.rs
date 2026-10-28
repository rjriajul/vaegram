use super::send_options::{SendOptions, caption_setters, send_builder};
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::types::messages_and_media::Source;
use crate::types::{FileSource, Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendPhoto<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    photo: FileSource<'a>,
    caption: String,
    parse_mode: Option<ParseMode>,
    options: SendOptions,
}

impl Client {
    pub fn send_photo<'a>(&'a self, chat_id: impl Into<PeerRef>, photo: impl Into<FileSource<'a>>) -> SendPhoto<'a> {
        SendPhoto {
            client: self,
            chat_id: chat_id.into(),
            photo: photo.into(),
            caption: String::new(),
            parse_mode: None,
            options: SendOptions::default(),
        }
    }
}

impl SendPhoto<'_> {
    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = caption.into();
        self
    }

    async fn send(self) -> Result<Message> {
        let media = match self.photo.0 {
            Source::Upload(upload) => {
                let file = self.client.save_file(upload).await?;
                base::InputMedia::InputMediaUploadedPhoto(Box::new(types::InputMediaUploadedPhoto {
                    spoiler: false,
                    live_photo: false,
                    file,
                    stickers: None,
                    ttl_seconds: None,
                    video: None,
                }))
            }
            Source::Stored(media @ base::InputMedia::InputMediaPhoto(_)) => media,
            Source::Stored(_) => return Err(Error::InvalidArgument("send_photo got a document; use send_document")),
        };
        self.client.send_media(self.chat_id, media, &self.caption, self.parse_mode, self.options).await
    }
}

caption_setters!(SendPhoto);
send_builder!(SendPhoto);
