use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::methods::{AnswerCallbackQuery, EditMessageText};
use crate::raw::types;
use crate::types::{Chat, User};

#[derive(Clone)]
pub struct CallbackQuery {
    pub id: i64,
    pub from_user: User,
    pub chat: Chat,
    pub message_id: i32,
    pub data: String,
    pub chat_instance: i64,
    client: Client,
}

impl CallbackQuery {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateBotCallbackQuery, entities: &Entities) -> CallbackQuery {
        CallbackQuery {
            id: raw.query_id,
            from_user: User::parse(raw.user_id, entities),
            chat: Chat::parse(&raw.peer, entities, false),
            message_id: raw.msg_id,
            data: raw.data.as_deref().map(String::from_utf8_lossy).unwrap_or_default().into_owned(),
            chat_instance: raw.chat_instance,
            client: client.clone(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn answer(&self) -> AnswerCallbackQuery<'_> {
        self.client.answer_callback_query(self.id)
    }

    pub fn edit_message_text(&self, text: impl Into<String>) -> EditMessageText<'_> {
        self.client.edit_message_text(self.chat.id, self.message_id, text)
    }
}

impl fmt::Debug for CallbackQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallbackQuery")
            .field("id", &self.id)
            .field("from_user", &self.from_user)
            .field("chat", &self.chat)
            .field("message_id", &self.message_id)
            .field("data", &self.data)
            .finish_non_exhaustive()
    }
}
