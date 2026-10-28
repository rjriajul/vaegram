use super::send_options::{SendOptions, caption_setters, send_builder};
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::session::transfer::Report;
use crate::types::messages_and_media::Source;
use crate::types::{FileSource, Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendDocument<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    document: FileSource<'a>,
    caption: String,
    parse_mode: Option<ParseMode>,
    options: SendOptions,
    progress: Option<Report<'a>>,
    force_file: bool,
}

impl Client {
    pub fn send_document<'a>(
        &'a self,
        chat_id: impl Into<PeerRef>,
        document: impl Into<FileSource<'a>>,
    ) -> SendDocument<'a> {
        SendDocument {
            client: self,
            chat_id: chat_id.into(),
            document: document.into(),
            caption: String::new(),
            parse_mode: None,
            options: SendOptions::default(),
            progress: None,
            force_file: false,
        }
    }
}

impl<'a> SendDocument<'a> {
    pub fn caption(mut self, caption: impl Into<String>) -> Self {
        self.caption = caption.into();
        self
    }

    pub fn progress(mut self, progress: impl FnMut(u64, u64) + Send + 'a) -> Self {
        self.progress = Some(Box::new(progress));
        self
    }

    /// Keeps a video, GIF or audio file a plain document instead of letting Telegram convert it.
    pub fn disable_content_type_detection(mut self) -> Self {
        self.force_file = true;
        self
    }

    async fn send(self) -> Result<Message> {
        let media = match self.document.0 {
            Source::Upload(upload) => {
                let file = self.client.save_file(upload).progress(self.progress).await?;
                let file_name = types::DocumentAttributeFilename { file_name: upload.name().into() };
                base::InputMedia::InputMediaUploadedDocument(Box::new(types::InputMediaUploadedDocument {
                    nosound_video: false,
                    force_file: self.force_file,
                    spoiler: false,
                    file,
                    thumb: None,
                    mime_type: upload.mime_type().into(),
                    attributes: vec![base::DocumentAttribute::DocumentAttributeFilename(Box::new(file_name))],
                    stickers: None,
                    video_cover: None,
                    video_timestamp: None,
                    ttl_seconds: None,
                }))
            }
            Source::Stored(media @ base::InputMedia::InputMediaDocument(_)) => media,
            Source::Stored(_) => return Err(Error::InvalidArgument("send_document got a photo; use send_photo")),
        };
        self.client.send_media(self.chat_id, media, &self.caption, self.parse_mode, self.options).await
    }
}

caption_setters!(SendDocument);
send_builder!(SendDocument);
