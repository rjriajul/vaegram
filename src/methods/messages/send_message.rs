use super::send_options::{SendOptions, caption_setters, send_builder};
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::base;
use crate::types::{Message, PeerRef, Text};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendMessage<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    text: Text,
    parse_mode: Option<ParseMode>,
    options: SendOptions,
}

impl Client {
    pub fn send_message(&self, chat_id: impl Into<PeerRef>, text: impl Into<Text>) -> SendMessage<'_> {
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
        self.write_text(text.into(), mode).await
    }

    pub(crate) async fn write_text(
        &self,
        text: Text,
        mode: Option<ParseMode>,
    ) -> Result<(String, Vec<base::MessageEntity>)> {
        let text = text.parsed(mode.unwrap_or(self.0.parse_mode));
        let mut entities = text.write_entities();
        for entity in &mut entities {
            if let base::MessageEntity::InputMessageEntityMentionName(m) = entity {
                if let base::InputUser::InputUser(u) = &m.user_id {
                    m.user_id = self.resolve_user(&u.user_id.into()).await?;
                }
            }
        }
        Ok((text.text, entities))
    }
}

impl SendMessage<'_> {
    /// Sends the text without a link preview (Pyrogram's `disable_web_page_preview`).
    pub fn disable_web_page_preview(mut self) -> Self {
        self.options.no_webpage = true;
        self
    }

    async fn send(self) -> Result<Message> {
        let (message, entities) = self.client.write_text(self.text, self.parse_mode).await?;
        let text = (message, (!entities.is_empty()).then_some(entities));
        self.client.send_parsed(self.chat_id, None, text, None, self.options).await
    }
}

caption_setters!(SendMessage);
send_builder!(SendMessage);
