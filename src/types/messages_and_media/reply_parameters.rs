use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::types;
use crate::types::Message;

#[derive(Debug, Clone, PartialEq)]
pub struct ReplyParameters {
    message_id: i32,
    chat_id: Option<i64>,
    quote: Option<String>,
    quote_parse_mode: Option<ParseMode>,
    quote_position: Option<i32>,
    checklist_task_id: Option<i32>,
}

impl ReplyParameters {
    pub fn new(message_id: i32) -> Self {
        ReplyParameters {
            message_id,
            chat_id: None,
            quote: None,
            quote_parse_mode: None,
            quote_position: None,
            checklist_task_id: None,
        }
    }

    pub fn chat_id(mut self, chat_id: i64) -> Self {
        self.chat_id = Some(chat_id);
        self
    }

    /// Quoted part of the replied message. It must match the message exactly, formatting included.
    pub fn quote(mut self, quote: impl Into<String>) -> Self {
        self.quote = Some(quote.into());
        self
    }

    pub fn quote_parse_mode(mut self, mode: ParseMode) -> Self {
        self.quote_parse_mode = Some(mode);
        self
    }

    pub fn quote_position(mut self, offset: i32) -> Self {
        self.quote_position = Some(offset);
        self
    }

    pub fn checklist_task_id(mut self, task_id: i32) -> Self {
        self.checklist_task_id = Some(task_id);
        self
    }

    pub(crate) async fn write(&self, client: &Client, chat_id: i64) -> Result<types::InputReplyToMessage> {
        let reply_to_peer_id = match self.chat_id {
            Some(id) if id != chat_id => Some(client.resolve_peer(id).await?),
            _ => None,
        };
        let (quote_text, quote_entities) = match &self.quote {
            Some(quote) => {
                let (text, entities) = client.parse_text(quote, self.quote_parse_mode).await?;
                (Some(text), (!entities.is_empty()).then_some(entities))
            }
            None => (None, None),
        };
        Ok(types::InputReplyToMessage {
            reply_to_msg_id: self.message_id,
            top_msg_id: None,
            reply_to_peer_id,
            quote_text,
            quote_entities,
            quote_offset: self.quote_position,
            monoforum_peer_id: None,
            todo_item_id: self.checklist_task_id,
            poll_option: None,
            quote_rich: None,
        })
    }
}

impl From<i32> for ReplyParameters {
    fn from(message_id: i32) -> Self {
        ReplyParameters::new(message_id)
    }
}

impl From<&Message> for ReplyParameters {
    fn from(message: &Message) -> Self {
        ReplyParameters::new(message.id).chat_id(message.chat.id)
    }
}
