use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InlineKeyboardMarkup, InlineMessageId};

impl Client {
    pub async fn edit_inline_reply_markup(
        &self,
        inline_message_id: InlineMessageId,
        reply_markup: Option<InlineKeyboardMarkup>,
    ) -> Result<bool> {
        let dc_id = inline_message_id.dc_id();
        let request = functions::messages::EditInlineBotMessage {
            no_webpage: false,
            invert_media: false,
            id: inline_message_id.0,
            message: None,
            media: None,
            reply_markup: reply_markup.as_ref().map(InlineKeyboardMarkup::write),
            entities: None,
            rich_message: None,
        };
        self.invoke_inline(dc_id, request).await
    }
}
