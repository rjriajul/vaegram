use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{InlineMessageId, InputMedia, ReplyMarkup};

#[must_use = "a request does nothing until it is awaited"]
/// Request built by [`Client::edit_inline_media`].
pub struct EditInlineMedia<'a> {
    client: &'a Client,
    inline_message_id: InlineMessageId,
    media: InputMedia<'a>,
    reply_markup: Option<ReplyMarkup>,
    business_connection_id: Option<String>,
}

impl Client {
    /// Replaces the media of an inline message.
    ///
    /// Takes only files already on Telegram, since an upload needs a chat to store it in. Media that needs an
    /// upload makes awaiting return `Error::InvalidArgument`. The caption in `media` is sent with it.
    pub fn edit_inline_media<'a>(
        &'a self,
        inline_message_id: InlineMessageId,
        media: InputMedia<'a>,
    ) -> EditInlineMedia<'a> {
        EditInlineMedia { client: self, inline_message_id, media, reply_markup: None, business_connection_id: None }
    }
}

impl EditInlineMedia<'_> {
    /// Attaches a reply markup to the edited message.
    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
        self
    }

    /// Edits a message sent through a business connection.
    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.business_connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<()> {
        let client = self.client;
        if self.media.uploads() {
            return Err(Error::InvalidArgument("an inline message can't take a new upload, only a stored file"));
        }
        let media = client.input_media(&base::InputPeer::InputPeerSelf, &self.media, None, None).await?;
        let (message, entities) = client.write_text(self.media.caption, self.media.parse_mode).await?;
        let dc_id = self.inline_message_id.dc_id();
        let request = functions::messages::EditInlineBotMessage {
            no_webpage: false,
            invert_media: self.media.show_caption_above_media,
            id: self.inline_message_id.0,
            message: Some(message),
            media: Some(media),
            reply_markup: ReplyMarkup::write_option(self.reply_markup.as_ref(), self.client).await?,
            entities: (!entities.is_empty()).then_some(entities),
            rich_message: None,
        };
        client.invoke_inline_on_behalf(dc_id, self.business_connection_id, request).await?;
        Ok(())
    }
}

into_future!(EditInlineMedia, ());
