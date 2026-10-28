use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{InlineKeyboardMarkup, Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct EditMessageReplyMarkup<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    reply_markup: Option<InlineKeyboardMarkup>,
    business_connection_id: Option<String>,
}

impl Client {
    pub fn edit_message_reply_markup(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        reply_markup: Option<InlineKeyboardMarkup>,
    ) -> EditMessageReplyMarkup<'_> {
        EditMessageReplyMarkup {
            client: self,
            chat_id: chat_id.into(),
            message_id,
            reply_markup,
            business_connection_id: None,
        }
    }
}

impl EditMessageReplyMarkup<'_> {
    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.business_connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<Message> {
        let client = self.client;
        let (chat_id, peer) = client.resolve(&self.chat_id).await?;
        let request = functions::messages::EditMessage {
            no_webpage: false,
            invert_media: false,
            peer,
            id: self.message_id,
            message: None,
            media: None,
            reply_markup: self.reply_markup.as_ref().map(InlineKeyboardMarkup::write),
            entities: None,
            schedule_date: None,
            schedule_repeat_period: None,
            quick_reply_shortcut_id: None,
            rich_message: None,
        };
        let (updates, _) = client.invoke_business(self.business_connection_id.as_deref(), request).await?;
        Message::from_updates(client, updates, chat_id, (String::new(), None))
    }
}

into_future!(EditMessageReplyMarkup, Message);

impl crate::methods::OnBehalf for EditMessageReplyMarkup<'_> {
    fn business_connection(&mut self) -> &mut Option<String> {
        &mut self.business_connection_id
    }
}
