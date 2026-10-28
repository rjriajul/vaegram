use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::methods::{AnswerCallbackQuery, EditEphemeralMessage, EditInlineText, EditMessageText, into_future};
use crate::raw::{base, types};
use crate::types::{Chat, EphemeralMessage, InlineKeyboardMarkup, InlineMessageId, MessageText, ReplyMarkup, User};
use crate::utils::get_peer;

#[derive(Clone)]
pub struct CallbackQuery {
    pub id: i64,
    pub from_user: User,
    pub origin: CallbackOrigin,
    pub data: String,
    /// Set when a `callback_game` button was pressed; answer with the game's URL to open it.
    pub game_short_name: Option<String>,
    pub chat_instance: i64,
    client: Client,
}

#[derive(Debug, Clone)]
pub enum CallbackOrigin {
    Message {
        chat: Box<Chat>,
        message_id: i32,
        /// Set for a business account's messages; edits then go through that connection.
        business_connection_id: Option<String>,
    },
    Inline(InlineMessageId),
    Ephemeral(Box<EphemeralMessage>),
}

impl CallbackQuery {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateBotCallbackQuery, entities: &Entities) -> CallbackQuery {
        CallbackQuery {
            id: raw.query_id,
            from_user: User::parse(raw.user_id, entities),
            origin: CallbackOrigin::Message {
                chat: Box::new(Chat::parse(&raw.peer, entities, false)),
                message_id: raw.msg_id,
                business_connection_id: None,
            },
            data: raw.data.as_deref().map(String::from_utf8_lossy).unwrap_or_default().into_owned(),
            game_short_name: raw.game_short_name.clone(),
            chat_instance: raw.chat_instance,
            client: client.clone(),
        }
    }

    pub(crate) fn parse_inline(
        client: &Client,
        raw: types::UpdateInlineBotCallbackQuery,
        entities: &Entities,
    ) -> CallbackQuery {
        CallbackQuery {
            id: raw.query_id,
            from_user: User::parse(raw.user_id, entities),
            origin: CallbackOrigin::Inline(InlineMessageId(raw.msg_id)),
            data: raw.data.map(utf8).unwrap_or_default(),
            game_short_name: raw.game_short_name,
            chat_instance: raw.chat_instance,
            client: client.clone(),
        }
    }

    pub(crate) fn parse_ephemeral(
        client: &Client,
        raw: types::UpdateEphemeralBotCallbackQuery,
        entities: &Entities,
    ) -> CallbackQuery {
        let mut message = EphemeralMessage::parse(client, raw.message, entities);
        if let Some(peer) = &raw.peer {
            message.chat = Chat::parse(peer, entities, false);
        }
        CallbackQuery {
            id: raw.query_id,
            from_user: User::parse(raw.user_id, entities),
            chat_instance: raw.chat_instance.or(message.raw.chat_instance).unwrap_or_default(),
            origin: CallbackOrigin::Ephemeral(Box::new(message)),
            data: utf8(raw.data),
            game_short_name: None,
            client: client.clone(),
        }
    }

    pub(crate) fn parse_business(
        client: &Client,
        raw: types::UpdateBusinessBotCallbackQuery,
        entities: &Entities,
    ) -> CallbackQuery {
        // Business chats are private, so a deleted message's chat is the user who pressed the button.
        let (peer, message_id) = match raw.message {
            base::Message::Message(m) => (m.peer_id, m.id),
            base::Message::MessageService(m) => (m.peer_id, m.id),
            base::Message::MessageEmpty(m) => (m.peer_id.unwrap_or_else(|| get_peer(raw.user_id)), m.id),
        };
        CallbackQuery {
            id: raw.query_id,
            from_user: User::parse(raw.user_id, entities),
            origin: CallbackOrigin::Message {
                chat: Box::new(Chat::parse(&peer, entities, false)),
                message_id,
                business_connection_id: Some(raw.connection_id),
            },
            data: raw.data.map(utf8).unwrap_or_default(),
            game_short_name: None,
            chat_instance: raw.chat_instance,
            client: client.clone(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn chat(&self) -> Option<&Chat> {
        match &self.origin {
            CallbackOrigin::Message { chat, .. } => Some(&**chat),
            CallbackOrigin::Ephemeral(message) => Some(&message.chat),
            CallbackOrigin::Inline(_) => None,
        }
    }

    pub fn message_id(&self) -> Option<i32> {
        match &self.origin {
            CallbackOrigin::Message { message_id, .. } => Some(*message_id),
            CallbackOrigin::Ephemeral(message) => Some(message.id),
            CallbackOrigin::Inline(_) => None,
        }
    }

    pub fn inline_message_id(&self) -> Option<&InlineMessageId> {
        if let CallbackOrigin::Inline(id) = &self.origin { Some(id) } else { None }
    }

    pub fn business_connection_id(&self) -> Option<&str> {
        match &self.origin {
            CallbackOrigin::Message { business_connection_id, .. } => business_connection_id.as_deref(),
            _ => None,
        }
    }

    pub fn answer(&self) -> AnswerCallbackQuery<'_> {
        self.client.answer_callback_query(self.id)
    }

    pub fn edit_message_text(&self, text: impl Into<MessageText>) -> EditCallbackMessageText<'_> {
        let client = &self.client;
        match &self.origin {
            CallbackOrigin::Message { chat, message_id, business_connection_id } => {
                let edit = client.edit_message_text(chat.id, *message_id, text);
                EditCallbackMessageText::Message(match business_connection_id {
                    Some(connection) => edit.business_connection_id(connection.clone()),
                    None => edit,
                })
            }
            CallbackOrigin::Inline(id) => EditCallbackMessageText::Inline(client.edit_inline_text(id.clone(), text)),
            CallbackOrigin::Ephemeral(message) => EditCallbackMessageText::Ephemeral(message.edit_text(text)),
        }
    }

    /// `None` removes the keyboard.
    pub async fn edit_message_reply_markup(&self, reply_markup: Option<InlineKeyboardMarkup>) -> Result<()> {
        match &self.origin {
            CallbackOrigin::Message { chat, message_id, business_connection_id } => {
                let edit = self.client.edit_message_reply_markup(chat.id, *message_id, reply_markup);
                match business_connection_id {
                    Some(connection) => edit.business_connection_id(connection.clone()).await.map(drop),
                    None => edit.await.map(drop),
                }
            }
            CallbackOrigin::Inline(id) => {
                self.client.edit_inline_reply_markup(id.clone(), reply_markup).await.map(drop)
            }
            CallbackOrigin::Ephemeral(m) => {
                let edit = self.client.edit_ephemeral_message_reply_markup(m.chat.id, m.receiver_user.id, m.id);
                match reply_markup {
                    Some(markup) => edit.reply_markup(markup).await.map(drop),
                    None => edit.await.map(drop),
                }
            }
        }
    }

    pub async fn delete_message(&self) -> Result<()> {
        match &self.origin {
            CallbackOrigin::Message { message_id, business_connection_id: Some(connection), .. } => {
                self.client.delete_business_messages(connection, &[*message_id]).await
            }
            CallbackOrigin::Message { chat, message_id, .. } => {
                self.client.delete_messages(chat.id, &[*message_id]).await
            }
            CallbackOrigin::Inline(_) => Err(Error::InvalidArgument("inline messages can't be deleted")),
            CallbackOrigin::Ephemeral(message) => message.delete().await.map(drop),
        }
    }
}

fn utf8(data: Vec<u8>) -> String {
    String::from_utf8(data).unwrap_or_else(|e| String::from_utf8_lossy(e.as_bytes()).into_owned())
}

impl fmt::Debug for CallbackQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CallbackQuery")
            .field("id", &self.id)
            .field("from_user", &self.from_user)
            .field("origin", &self.origin)
            .field("data", &self.data)
            .field("game_short_name", &self.game_short_name)
            .finish_non_exhaustive()
    }
}

#[must_use = "a request does nothing until it is awaited"]
pub enum EditCallbackMessageText<'a> {
    Message(EditMessageText<'a>),
    Inline(EditInlineText<'a>),
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
            Self::Inline(edit) => Self::Inline(edit.parse_mode(mode)),
            Self::Ephemeral(edit) => Self::Ephemeral(edit.parse_mode(mode)),
        }
    }

    pub fn reply_markup(self, markup: impl Into<ReplyMarkup>) -> Self {
        match self {
            Self::Message(edit) => Self::Message(edit.reply_markup(markup)),
            Self::Inline(edit) => Self::Inline(edit.reply_markup(markup)),
            Self::Ephemeral(edit) => Self::Ephemeral(edit.reply_markup(markup)),
        }
    }

    async fn send(self) -> Result<()> {
        match self {
            Self::Message(edit) => edit.await.map(drop),
            Self::Inline(edit) => edit.await.map(drop),
            Self::Ephemeral(edit) => edit.await.map(drop),
        }
    }
}

into_future!(EditCallbackMessageText, ());

#[cfg(test)]
mod tests {
    use super::{CallbackOrigin, CallbackQuery, EditCallbackMessageText};
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::errors::Error;
    use crate::raw::functions::messages;
    use crate::raw::{Identifiable, base, types};
    use crate::types::Message;
    use crate::types::messages_and_media::ephemeral_message::tests::{channel, ephemeral};
    use crate::utils::{get_channel_id, get_peer};

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
        let chat_id = q.chat().map(|c| c.id);
        assert_eq!((q.id, q.from_user.id, chat_id, q.data.as_str()), (9, 1000, Some(get_channel_id(42)), "tap"));
        assert_eq!(q.message_id(), Some(7));
        assert!(matches!(q.edit_message_text("tapped"), EditCallbackMessageText::Ephemeral(_)));
    }

    #[test]
    fn inline_game_callbacks_edit_the_inline_message() {
        let id = types::InputBotInlineMessageID64 { dc_id: 2, owner_id: 5, id: 6, access_hash: 7 };
        let update = types::UpdateInlineBotCallbackQuery {
            query_id: 1,
            user_id: 1000,
            msg_id: base::InputBotInlineMessageID::InputBotInlineMessageID64(Box::new(id)),
            chat_instance: 3,
            data: None,
            game_short_name: Some("race".into()),
        };
        let q = CallbackQuery::parse_inline(&Client::offline(), update, &Entities::default());
        assert_eq!((q.chat().is_none(), q.message_id(), q.game_short_name.as_deref()), (true, None, Some("race")));
        assert_eq!(q.inline_message_id().map(|i| i.dc_id()), Some(2));
        assert!(matches!(q.edit_message_text("won"), EditCallbackMessageText::Inline(_)));
    }

    #[tokio::test]
    async fn business_callback_queries_edit_through_the_connection() {
        let message = Message::bare(5, get_peer(7), 0);
        let update = types::UpdateBusinessBotCallbackQuery {
            query_id: 1,
            user_id: 7,
            connection_id: "conn".into(),
            message: base::Message::Message(Box::new(message)),
            reply_to_message: None,
            chat_instance: 3,
            data: Some(b"buy".to_vec()),
        };
        let (client, mut rx) = Client::recording();
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(7, peer);
        let q = CallbackQuery::parse_business(&client, update.clone(), &Entities::default());
        let chat_id = q.chat().map(|c| c.id);
        assert_eq!((chat_id, q.message_id(), q.from_user.id, q.data.as_str()), (Some(7), Some(5), 7, "buy"));
        assert_eq!(q.business_connection_id(), Some("conn"));
        assert!(q.edit_message_text("bought").await.is_err());
        let body = rx.try_recv().expect("no request sent");
        assert_eq!(body[..4], 0xdd289f8e_u32.to_le_bytes());

        let message = base::Message::MessageEmpty(Box::new(types::MessageEmpty { id: 5, peer_id: None }));
        let q = CallbackQuery::parse_business(
            &Client::offline(),
            types::UpdateBusinessBotCallbackQuery { message, ..update },
            &Entities::default(),
        );
        assert_eq!((q.chat().map(|c| c.id), q.message_id()), (Some(7), Some(5)));
    }

    #[tokio::test]
    async fn delete_and_keyboard_edits_follow_the_origin() {
        let (client, mut rx) = Client::recording();
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 7, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(7, peer);
        let update = types::UpdateBusinessBotCallbackQuery {
            query_id: 1,
            user_id: 7,
            connection_id: "conn".into(),
            message: base::Message::Message(Box::new(Message::bare(5, get_peer(7), 0))),
            reply_to_message: None,
            chat_instance: 3,
            data: None,
        };
        let mut q = CallbackQuery::parse_business(&client, update, &Entities::default());
        let _ = q.delete_message().await;
        if let CallbackOrigin::Message { business_connection_id, .. } = &mut q.origin {
            *business_connection_id = None;
        }
        let _ = q.delete_message().await;
        let mut sent = || u32::from_le_bytes(rx.try_recv().expect("no request sent")[..4].try_into().unwrap());
        assert_eq!((sent(), sent()), (0xdd289f8e, messages::DeleteMessages::CONSTRUCTOR_ID));

        let id = types::InputBotInlineMessageID64 { dc_id: 2, owner_id: 5, id: 6, access_hash: 7 };
        let update = types::UpdateInlineBotCallbackQuery {
            query_id: 1,
            user_id: 1000,
            msg_id: base::InputBotInlineMessageID::InputBotInlineMessageID64(Box::new(id)),
            chat_instance: 3,
            data: None,
            game_short_name: None,
        };
        let q = CallbackQuery::parse_inline(&client, update, &Entities::default());
        assert!(matches!(q.delete_message().await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
        let _ = q.edit_message_reply_markup(None).await;
        let sent = u32::from_le_bytes(rx.try_recv().expect("no request sent")[..4].try_into().unwrap());
        assert_eq!(sent, messages::EditInlineBotMessage::CONSTRUCTOR_ID);
    }
}
