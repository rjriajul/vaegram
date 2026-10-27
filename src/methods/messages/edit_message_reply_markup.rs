use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InlineKeyboardMarkup, Message};

impl Client {
    pub async fn edit_message_reply_markup(
        &self,
        chat_id: i64,
        message_id: i32,
        reply_markup: Option<InlineKeyboardMarkup>,
    ) -> Result<Message> {
        let request = functions::messages::EditMessage {
            no_webpage: false,
            invert_media: false,
            peer: self.resolve_peer(chat_id).await?,
            id: message_id,
            message: None,
            media: None,
            reply_markup: reply_markup.as_ref().map(InlineKeyboardMarkup::write),
            entities: None,
            schedule_date: None,
            schedule_repeat_period: None,
            quick_reply_shortcut_id: None,
            rich_message: None,
        };
        let updates = self.invoke(&request).await?;
        Message::from_updates(self, updates, chat_id, (String::new(), None))
    }
}
