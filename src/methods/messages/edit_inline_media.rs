use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{InlineMessageId, InputMedia, ReplyMarkup};

#[must_use = "a request does nothing until it is awaited"]
pub struct EditInlineMedia<'a> {
    client: &'a Client,
    inline_message_id: InlineMessageId,
    media: InputMedia<'a>,
    reply_markup: Option<ReplyMarkup>,
}

impl Client {
    /// Only files already on Telegram: an upload would first need a chat to store it in, as in the Bot API.
    pub fn edit_inline_media<'a>(
        &'a self,
        inline_message_id: InlineMessageId,
        media: InputMedia<'a>,
    ) -> EditInlineMedia<'a> {
        EditInlineMedia { client: self, inline_message_id, media, reply_markup: None }
    }
}

impl EditInlineMedia<'_> {
    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
        self
    }

    async fn send(self) -> Result<bool> {
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
            reply_markup: self.reply_markup.as_ref().map(ReplyMarkup::write),
            entities: (!entities.is_empty()).then_some(entities),
            rich_message: None,
        };
        client.invoke_inline(dc_id, request).await
    }
}

into_future!(EditInlineMedia, bool);
