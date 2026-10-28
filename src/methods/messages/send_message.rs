use super::send_options::{SendOptions, caption_setters, send_builder};
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::parser;
use crate::raw::base;
use crate::types::{Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendMessage<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    text: String,
    parse_mode: Option<ParseMode>,
    options: SendOptions,
}

impl Client {
    pub fn send_message(&self, chat_id: impl Into<PeerRef>, text: impl Into<String>) -> SendMessage<'_> {
        SendMessage {
            client: self,
            chat_id: chat_id.into(),
            text: text.into(),
            parse_mode: None,
            options: SendOptions::default(),
        }
    }

    pub(crate) async fn parse_text(
        &self,
        text: &str,
        mode: Option<ParseMode>,
    ) -> Result<(String, Vec<base::MessageEntity>)> {
        let (text, mut entities) = parser::parse(text, mode.unwrap_or(self.0.parse_mode));
        for entity in &mut entities {
            if let base::MessageEntity::InputMessageEntityMentionName(m) = entity {
                if let base::InputUser::InputUser(u) = &m.user_id {
                    m.user_id = self.resolve_user(&u.user_id.into()).await?;
                }
            }
        }
        Ok((text, entities))
    }
}

impl SendMessage<'_> {
    async fn send(self) -> Result<Message> {
        let (message, entities) = self.client.parse_text(&self.text, self.parse_mode).await?;
        let text = (message, (!entities.is_empty()).then_some(entities));
        self.client.send_parsed(self.chat_id, None, text, None, self.options).await
    }
}

caption_setters!(SendMessage);
send_builder!(SendMessage);
