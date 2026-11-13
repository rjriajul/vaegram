use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::methods::{AnswerCallbackQuery, EditCallbackMessageText, SendEphemeralMessage};
use crate::raw::{base, types};
use crate::types::{Chat, EphemeralMessage, InlineKeyboardMarkup, InlineMessageId, Message, MessageText, Text, User};
use crate::utils::get_peer;

/// A press of an inline keyboard button that carries callback data.
#[derive(Clone)]
pub struct CallbackQuery {
    /// The query id, used to answer it.
    pub id: i64,
    /// The user who pressed the button.
    pub from_user: User,
    /// The message the button is on.
    pub origin: CallbackOrigin,
    /// The data attached to the button, empty if it has none. Invalid UTF-8 is replaced.
    pub data: String,
    /// Set when a `callback_game` button was pressed. Answer with the game's URL to open it.
    pub game_short_name: Option<String>,
    /// Identifies the chat the message was shown in.
    pub chat_instance: i64,
    // A business update carries the pressed message, since the bot can't read the account's history.
    carried: Option<Box<Message>>,
    client: Client,
}

/// Where the pressed button lives.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum CallbackOrigin {
    /// A regular message in a chat.
    #[non_exhaustive]
    Message {
        /// The chat the message is in.
        chat: Box<Chat>,
        /// Id of the message within the chat.
        message_id: i32,
        /// Set for a business account's messages. Edits then go through that connection.
        business_connection_id: Option<String>,
    },
    /// A message sent through inline mode.
    Inline(InlineMessageId),
    /// An ephemeral message.
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
            carried: None,
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
            data: raw.data.as_deref().map(String::from_utf8_lossy).unwrap_or_default().into_owned(),
            game_short_name: raw.game_short_name,
            chat_instance: raw.chat_instance,
            carried: None,
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
            data: String::from_utf8_lossy(&raw.data).into_owned(),
            game_short_name: None,
            carried: None,
            client: client.clone(),
        }
    }

    pub(crate) fn parse_business(
        client: &Client,
        raw: types::UpdateBusinessBotCallbackQuery,
        entities: &Entities,
    ) -> CallbackQuery {
        // Business chats are private, so a deleted message's chat is the user who pressed the button.
        let (peer, message_id) = match &raw.message {
            base::Message::Message(m) => (m.peer_id.clone(), m.id),
            base::Message::MessageService(m) => (m.peer_id.clone(), m.id),
            base::Message::MessageEmpty(m) => (m.peer_id.clone().unwrap_or_else(|| get_peer(raw.user_id)), m.id),
        };
        let mut carried = Message::from_raw(client, raw.message, entities);
        if let Some(message) = carried.as_mut() {
            message.set_business(client, raw.connection_id.clone(), raw.reply_to_message, entities);
        }
        CallbackQuery {
            id: raw.query_id,
            from_user: User::parse(raw.user_id, entities),
            origin: CallbackOrigin::Message {
                chat: Box::new(Chat::parse(&peer, entities, false)),
                message_id,
                business_connection_id: Some(raw.connection_id),
            },
            data: raw.data.as_deref().map(String::from_utf8_lossy).unwrap_or_default().into_owned(),
            game_short_name: None,
            chat_instance: raw.chat_instance,
            carried: carried.map(Box::new),
            client: client.clone(),
        }
    }

    /// The client that received the update.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Returns the message the button was pressed on.
    ///
    /// A business press already carries it. A regular one comes from the message cache when it's on and is
    /// fetched otherwise, so `None` means it was deleted since. Inline and ephemeral presses have no regular
    /// message and return `Ok(None)`.
    pub async fn message(&self) -> Result<Option<Message>> {
        match &self.origin {
            CallbackOrigin::Message { chat, message_id, business_connection_id } => {
                if business_connection_id.is_some() {
                    return Ok(self.carried.as_deref().cloned());
                }
                let cached = self.client.0.caches.message(chat.id, *message_id);
                if let Some(message) = cached.and_then(|c| Message::from_raw(&self.client, c.raw, &c.entities)) {
                    return Ok(Some(message));
                }
                Ok(self.client.get_messages(chat.id, &[*message_id]).await?.into_iter().next().flatten())
            }
            CallbackOrigin::Inline(_) | CallbackOrigin::Ephemeral(_) => Ok(None),
        }
    }

    /// The chat of the message. `None` for inline messages.
    pub fn chat(&self) -> Option<&Chat> {
        match &self.origin {
            CallbackOrigin::Message { chat, .. } => Some(&**chat),
            CallbackOrigin::Ephemeral(message) => Some(&message.chat),
            CallbackOrigin::Inline(_) => None,
        }
    }

    /// Id of the message. `None` for inline messages.
    pub fn message_id(&self) -> Option<i32> {
        match &self.origin {
            CallbackOrigin::Message { message_id, .. } => Some(*message_id),
            CallbackOrigin::Ephemeral(message) => Some(message.id),
            CallbackOrigin::Inline(_) => None,
        }
    }

    /// The inline message id. `None` unless the button is on an inline message.
    pub fn inline_message_id(&self) -> Option<&InlineMessageId> {
        if let CallbackOrigin::Inline(id) = &self.origin { Some(id) } else { None }
    }

    /// The business connection of the message. `None` unless it belongs to a business account.
    pub fn business_connection_id(&self) -> Option<&str> {
        match &self.origin {
            CallbackOrigin::Message { business_connection_id, .. } => business_connection_id.as_deref(),
            _ => None,
        }
    }

    /// Starts an answer to this query. See [`Client::answer_callback_query`].
    pub fn answer(&self) -> AnswerCallbackQuery<'_> {
        self.client.answer_callback_query(self.id)
    }

    /// Sends an ephemeral message to the user who pressed the button, answering this query.
    ///
    /// Fails with [`Error::InvalidArgument`] if the button is on an inline message.
    pub fn answer_ephemeral(&self, text: impl Into<Text>) -> Result<SendEphemeralMessage<'_>> {
        let chat =
            self.chat().ok_or(Error::InvalidArgument("inline callback queries can't receive ephemeral messages"))?;
        Ok(self.client.send_ephemeral_message(chat.id, self.from_user.id, text).callback_query_id(self.id))
    }

    /// Edits the text of the message the button is on.
    ///
    /// Works for regular, inline and ephemeral messages. A business account's message is edited through its
    /// connection.
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

    /// Replaces the inline keyboard of the message the button is on. `None` removes the keyboard.
    pub async fn edit_message_reply_markup(&self, reply_markup: Option<InlineKeyboardMarkup>) -> Result<()> {
        match &self.origin {
            CallbackOrigin::Message { chat, message_id, business_connection_id } => {
                let edit = self.client.edit_message_reply_markup(chat.id, *message_id, reply_markup);
                match business_connection_id {
                    Some(connection) => edit.business_connection_id(connection.clone()).await.map(drop),
                    None => edit.await.map(drop),
                }
            }
            CallbackOrigin::Inline(id) => self.client.edit_inline_reply_markup(id.clone(), reply_markup).await,
            CallbackOrigin::Ephemeral(m) => {
                let edit = self.client.edit_ephemeral_message_reply_markup(m.chat.id, m.receiver_user.id, m.id);
                match reply_markup {
                    Some(markup) => edit.reply_markup(markup).await.map(drop),
                    None => edit.await.map(drop),
                }
            }
        }
    }

    /// Deletes the message the button is on. Fails with `Error::InvalidArgument` for an inline message.
    pub async fn delete_message(&self) -> Result<()> {
        match &self.origin {
            CallbackOrigin::Message { message_id, business_connection_id: Some(connection), .. } => {
                self.client.delete_business_messages(connection, &[*message_id]).await
            }
            CallbackOrigin::Message { chat, message_id, .. } => {
                self.client.delete_messages(chat.id, &[*message_id]).await
            }
            CallbackOrigin::Inline(_) => Err(Error::InvalidArgument("inline messages can't be deleted")),
            CallbackOrigin::Ephemeral(message) => message.delete().await,
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{CallbackOrigin, CallbackQuery};
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::errors::Error;
    use crate::methods::EditCallbackMessageText;
    use crate::raw::functions::messages;
    use crate::raw::{Identifiable, Serializable, base, types};
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

    #[tokio::test]
    async fn answer_ephemeral_sends_to_the_presser_with_the_query_id() {
        let (client, mut sent) = Client::recording();
        let input_channel = types::InputPeerChannel { channel_id: 42, access_hash: 7 };
        let user = types::InputPeerUser { user_id: 1000, access_hash: 9 };
        {
            let mut peers = client.0.peers.lock().unwrap();
            peers.insert(get_channel_id(42), base::InputPeer::InputPeerChannel(Box::new(input_channel)));
            peers.insert(1000, base::InputPeer::InputPeerUser(Box::new(user)));
        }
        let update = types::UpdateEphemeralBotCallbackQuery {
            query_id: 9,
            user_id: 1000,
            peer: None,
            msg_id: 7,
            data: b"tap".to_vec(),
            chat_instance: None,
            message: ephemeral(true, Some(channel())),
        };
        let q = CallbackQuery::parse_ephemeral(&client, update, &Entities::default());
        let send = q.answer_ephemeral("reply").unwrap();
        let _ = send.await;
        let body = sent.try_recv().expect("no request sent");
        // peer, receiver_id, query_id and message are adjacent in ephemeral.sendMessage.
        let peer =
            base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id: 42, access_hash: 7 }));
        let receiver = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 1000, access_hash: 9 }));
        let mut expected = peer.to_bytes();
        expected.extend(receiver.to_bytes());
        expected.extend(9i64.to_bytes());
        expected.extend(String::from("reply").to_bytes());
        assert!(body.windows(expected.len()).any(|w| w == expected));
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
        client.0.caches.know_business("conn", 7, 2);
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
        client.0.caches.know_business("conn", 7, 2);
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
        assert!(matches!(q.answer_ephemeral("hi"), Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
        let _ = q.edit_message_reply_markup(None).await;
        let sent = u32::from_le_bytes(rx.try_recv().expect("no request sent")[..4].try_into().unwrap());
        assert_eq!(sent, messages::EditInlineBotMessage::CONSTRUCTOR_ID);
    }

    #[tokio::test]
    async fn business_press_returns_the_carried_message() {
        let (client, mut rx) = Client::recording();
        let update = types::UpdateBusinessBotCallbackQuery {
            query_id: 1,
            user_id: 7,
            connection_id: "conn".into(),
            message: base::Message::Message(Box::new(Message::bare(5, get_peer(7), 0))),
            reply_to_message: Some(base::Message::Message(Box::new(Message::bare(4, get_peer(7), 0)))),
            chat_instance: 3,
            data: None,
        };
        let q = CallbackQuery::parse_business(&client, update, &Entities::default());
        let message = q.message().await.unwrap().expect("the update carries the message");
        assert_eq!((message.id, message.business_connection_id.as_deref()), (5, Some("conn")));
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn message_press_fetches_the_message() {
        let found = types::messages::Messages {
            messages: vec![base::Message::Message(Box::new(Message::bare(5, get_peer(7), 0)))],
            topics: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        let reply = base::messages::Messages::Messages(Box::new(found)).to_bytes();
        let (client, mut rx) = Client::scripted(vec![Ok(reply)]);
        let q = crate::dispatcher::listeners::tests::query(&client, 7, 7, 5);
        let message = q.message().await.unwrap().expect("the message exists");
        assert_eq!((message.id, message.chat.id), (5, 7));
        let id = base::InputMessage::InputMessageID(types::InputMessageID { id: 5 });
        assert_eq!(rx.try_recv().unwrap(), messages::GetMessages { id: vec![id] }.to_bytes());

        let inline = crate::dispatcher::listeners::tests::inline_query(&client, 7, 1);
        assert!(inline.message().await.unwrap().is_none());
        assert!(rx.try_recv().is_err());
    }
}
