use super::send_options::caption_setters;
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InlineMessageId, MessageText, ReplyMarkup};

#[must_use = "a request does nothing until it is awaited"]
pub struct EditInlineText<'a> {
    client: &'a Client,
    inline_message_id: InlineMessageId,
    text: MessageText,
    parse_mode: Option<ParseMode>,
    reply_markup: Option<ReplyMarkup>,
    no_webpage: bool,
}

impl Client {
    pub fn edit_inline_text(
        &self,
        inline_message_id: InlineMessageId,
        text: impl Into<MessageText>,
    ) -> EditInlineText<'_> {
        EditInlineText {
            client: self,
            inline_message_id,
            text: text.into(),
            parse_mode: None,
            reply_markup: None,
            no_webpage: false,
        }
    }
}

impl EditInlineText<'_> {
    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
        self
    }

    /// Sends the text without a link preview (Pyrogram's `disable_web_page_preview`).
    pub fn disable_web_page_preview(mut self) -> Self {
        self.no_webpage = true;
        self
    }

    async fn send(self) -> Result<bool> {
        let client = self.client;
        let (message, entities, rich_message) = self.text.write(client, self.parse_mode).await?;
        let dc_id = self.inline_message_id.dc_id();
        let request = functions::messages::EditInlineBotMessage {
            no_webpage: self.no_webpage,
            invert_media: false,
            id: self.inline_message_id.0,
            message,
            media: None,
            reply_markup: self.reply_markup.as_ref().map(ReplyMarkup::write),
            entities,
            rich_message,
        };
        client.invoke_inline(dc_id, request).await
    }
}

caption_setters!(EditInlineText);
into_future!(EditInlineText, bool);
