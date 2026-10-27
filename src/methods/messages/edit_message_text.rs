use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Message, ReplyMarkup};

pub struct EditMessageText<'a> {
    client: &'a Client,
    chat_id: i64,
    message_id: i32,
    text: String,
    parse_mode: ParseMode,
    reply_markup: Option<ReplyMarkup>,
}

impl Client {
    pub fn edit_message_text(&self, chat_id: i64, message_id: i32, text: impl Into<String>) -> EditMessageText<'_> {
        EditMessageText {
            client: self,
            chat_id,
            message_id,
            text: text.into(),
            parse_mode: ParseMode::Disabled,
            reply_markup: None,
        }
    }
}

impl EditMessageText<'_> {
    pub fn html(self) -> Self {
        self.parse_mode(ParseMode::Html)
    }

    pub fn markdown(self) -> Self {
        self.parse_mode(ParseMode::Markdown)
    }

    pub fn parse_mode(mut self, mode: ParseMode) -> Self {
        self.parse_mode = mode;
        self
    }

    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
        self
    }

    async fn send(self) -> Result<Message> {
        let client = self.client;
        let peer = client.resolve_peer(self.chat_id).await?;
        let (message, entities) = client.parse_text(&self.text, self.parse_mode).await?;
        let entities = (!entities.is_empty()).then_some(entities);
        let request = functions::messages::EditMessage {
            no_webpage: false,
            invert_media: false,
            peer,
            id: self.message_id,
            message: Some(message.clone()),
            media: None,
            reply_markup: self.reply_markup.as_ref().map(ReplyMarkup::write),
            entities: entities.clone(),
            schedule_date: None,
            schedule_repeat_period: None,
            quick_reply_shortcut_id: None,
            rich_message: None,
        };
        let updates = client.invoke(&request).await?;
        Message::from_updates(client, updates, self.chat_id, (message, entities))
    }
}

into_future!(EditMessageText, Message);
