use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InlineKeyboardMarkup, InlineMessageId};

#[must_use = "a request does nothing until it is awaited"]
/// Request built by [`Client::edit_inline_reply_markup`]. Awaiting it returns Telegram's `bool` answer.
pub struct EditInlineReplyMarkup<'a> {
    client: &'a Client,
    inline_message_id: InlineMessageId,
    reply_markup: Option<InlineKeyboardMarkup>,
    business_connection_id: Option<String>,
}

impl Client {
    /// Replaces the inline keyboard of an inline message. `None` removes the keyboard.
    pub fn edit_inline_reply_markup(
        &self,
        inline_message_id: InlineMessageId,
        reply_markup: Option<InlineKeyboardMarkup>,
    ) -> EditInlineReplyMarkup<'_> {
        EditInlineReplyMarkup { client: self, inline_message_id, reply_markup, business_connection_id: None }
    }
}

impl EditInlineReplyMarkup<'_> {
    /// Edits a message sent through a business connection.
    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.business_connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<bool> {
        let dc_id = self.inline_message_id.dc_id();
        let request = functions::messages::EditInlineBotMessage {
            no_webpage: false,
            invert_media: false,
            id: self.inline_message_id.0,
            message: None,
            media: None,
            reply_markup: InlineKeyboardMarkup::write_option(self.reply_markup.as_ref(), self.client).await?,
            entities: None,
            rich_message: None,
        };
        self.client.invoke_inline_on_behalf(dc_id, self.business_connection_id, request).await
    }
}

into_future!(EditInlineReplyMarkup, bool);
