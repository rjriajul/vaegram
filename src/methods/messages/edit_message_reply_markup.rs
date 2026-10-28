use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InlineKeyboardMarkup, Message, PeerRef};

impl Client {
    pub async fn edit_message_reply_markup(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        reply_markup: Option<InlineKeyboardMarkup>,
    ) -> Result<Message> {
        let (chat_id, peer) = self.resolve(&chat_id.into()).await?;
        let request = functions::messages::EditMessage {
            no_webpage: false,
            invert_media: false,
            peer,
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
