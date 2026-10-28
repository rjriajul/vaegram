use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::methods::{AnswerCallbackQuery, EditEphemeralMessage, EditMessageText, into_future};
use crate::raw::types;
use crate::types::{Chat, EphemeralMessage, MessageText, ReplyMarkup, User};

#[derive(Clone)]
pub struct CallbackQuery {
    pub id: i64,
    pub from_user: User,
    pub chat: Chat,
    pub message_id: i32,
    pub data: String,
    pub chat_instance: i64,
    /// Set when the button belongs to an ephemeral message; `message_id` is then its ephemeral id.
    pub ephemeral_message: Option<Box<EphemeralMessage>>,
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
            ephemeral_message: None,
            client: client.clone(),
        }
    }

    pub(crate) fn parse_ephemeral(
        client: &Client,
        raw: types::UpdateEphemeralBotCallbackQuery,
        entities: &Entities,
    ) -> CallbackQuery {
        let message = EphemeralMessage::parse(client, raw.message, entities);
        CallbackQuery {
            id: raw.query_id,
            from_user: User::parse(raw.user_id, entities),
            chat: raw.peer.as_ref().map_or_else(|| message.chat.clone(), |peer| Chat::parse(peer, entities, false)),
            message_id: raw.msg_id,
            data: String::from_utf8(raw.data).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned()),
            chat_instance: raw.chat_instance.or(message.raw.chat_instance).unwrap_or_default(),
            ephemeral_message: Some(Box::new(message)),
            client: client.clone(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn answer(&self) -> AnswerCallbackQuery<'_> {
        self.client.answer_callback_query(self.id)
    }

    pub fn edit_message_text(&self, text: impl Into<MessageText>) -> EditCallbackMessageText<'_> {
        match &self.ephemeral_message {
            Some(message) => EditCallbackMessageText::Ephemeral(message.edit_text(text)),
            None => {
                EditCallbackMessageText::Message(self.client.edit_message_text(self.chat.id, self.message_id, text))
            }
        }
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
            .field("ephemeral_message", &self.ephemeral_message)
            .finish_non_exhaustive()
    }
}

#[must_use = "a request does nothing until it is awaited"]
pub enum EditCallbackMessageText<'a> {
    Message(EditMessageText<'a>),
    Ephemeral(EditEphemeralMessage<'a>),
}

impl EditCallbackMessageText<'_> {
    pub fn html(self) -> Self {
        self.parse_mode(ParseMode::Html)
    }

    pub fn markdown(self) -> Self {
        self.parse_mode(ParseMode::Markdown)
    }

    pub fn parse_mode(self, mode: ParseMode) -> Self {
        match self {
            Self::Message(edit) => Self::Message(edit.parse_mode(mode)),
            Self::Ephemeral(edit) => Self::Ephemeral(edit.parse_mode(mode)),
        }
    }

    pub fn reply_markup(self, markup: impl Into<ReplyMarkup>) -> Self {
        match self {
            Self::Message(edit) => Self::Message(edit.reply_markup(markup)),
            Self::Ephemeral(edit) => Self::Ephemeral(edit.reply_markup(markup)),
        }
    }

    async fn send(self) -> Result<()> {
        match self {
            Self::Message(edit) => edit.await.map(drop),
            Self::Ephemeral(edit) => edit.await.map(drop),
        }
    }
}

into_future!(EditCallbackMessageText, ());

#[cfg(test)]
mod tests {
    use super::{CallbackQuery, EditCallbackMessageText};
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::types;
    use crate::types::messages_and_media::ephemeral_message::tests::{channel, ephemeral};
    use crate::utils::get_channel_id;

    #[test]
    fn parses_ephemeral_callback_queries() {
        let update = types::UpdateEphemeralBotCallbackQuery {
            query_id: 9,
            user_id: 1000,
            peer: None,
            msg_id: 7,
            data: b"tap".to_vec(),
            chat_instance: None,
            message: ephemeral(true, Some(channel())),
        };
        let q = CallbackQuery::parse_ephemeral(&Client::offline(), update, &Entities::default());
        assert_eq!((q.id, q.from_user.id, q.chat.id, q.data.as_str()), (9, 1000, get_channel_id(42), "tap"));
        assert_eq!(q.ephemeral_message.as_ref().map(|m| m.id), Some(7));
        assert!(matches!(q.edit_message_text("tapped"), EditCallbackMessageText::Ephemeral(_)));
    }
}
