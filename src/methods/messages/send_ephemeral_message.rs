use super::send_options::{SendOptions, caption_setters};
use crate::Client;
use crate::crypto::random;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::{EphemeralMessage, InputMedia, MessageText, PeerRef, ReplyMarkup, ReplyParameters, Text};

enum ReplyTo {
    Message(ReplyParameters),
    Ephemeral(i32),
}

/// Request built by [`Client::send_ephemeral_message`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SendEphemeralMessage<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    receiver_user_id: Option<PeerRef>,
    text: MessageText,
    parse_mode: Option<ParseMode>,
    media: Option<InputMedia<'a>>,
    protect_content: bool,
    callback_query_id: Option<i64>,
    replace: bool,
    reply_to: Option<ReplyTo>,
    message_thread_id: Option<i32>,
    reply_markup: Option<ReplyMarkup>,
}

impl Client {
    /// Sends a message in a chat that only `receiver_user_id` sees.
    ///
    /// The text is plain or formatted [`Text`], or a rich message built with
    /// [`InputRichMessage`](crate::types::InputRichMessage). Awaiting the request returns the sent
    /// [`EphemeralMessage`].
    pub fn send_ephemeral_message(
        &self,
        chat_id: impl Into<PeerRef>,
        receiver_user_id: impl Into<PeerRef>,
        text: impl Into<MessageText>,
    ) -> SendEphemeralMessage<'_> {
        SendEphemeralMessage::new(self, chat_id.into(), Some(receiver_user_id.into()), text.into())
    }
}

impl<'a> SendEphemeralMessage<'a> {
    /// A request for `receiver_user_id`. Without one, awaiting fails with `Error::InvalidArgument` before any
    /// request is sent.
    pub(crate) fn new(
        client: &'a Client,
        chat_id: PeerRef,
        receiver_user_id: Option<PeerRef>,
        text: MessageText,
    ) -> Self {
        SendEphemeralMessage {
            client,
            chat_id,
            receiver_user_id,
            text,
            parse_mode: None,
            media: None,
            protect_content: false,
            callback_query_id: None,
            replace: false,
            reply_to: None,
            message_thread_id: None,
            reply_markup: None,
        }
    }

    /// Answers the callback query with this id by sending the message.
    pub fn callback_query_id(mut self, query_id: i64) -> Self {
        self.callback_query_id = Some(query_id);
        self
    }

    /// Replaces the message the callback query's button is on instead of sending a new one.
    ///
    /// Without [`callback_query_id`](Self::callback_query_id), awaiting fails with `Error::InvalidArgument`.
    pub fn replace_callback_query_message(mut self) -> Self {
        self.replace = true;
        self
    }

    /// Attaches media to the message.
    ///
    /// The caption comes from `InputMedia::caption`. The message text must then be empty, or awaiting fails with
    /// `Error::InvalidArgument`; a rich message can't carry media at all.
    pub fn media(mut self, media: InputMedia<'a>) -> Self {
        self.media = Some(media);
        self
    }

    /// Stops the receiver from forwarding or saving the message.
    pub fn protect_content(mut self) -> Self {
        self.protect_content = true;
        self
    }

    /// Replies to a regular message.
    pub fn reply_to(mut self, reply: impl Into<ReplyParameters>) -> Self {
        self.reply_to = Some(ReplyTo::Message(reply.into()));
        self
    }

    /// Replies to the ephemeral message with this id.
    ///
    /// A thread can't be named as well: `ephemeral.sendMessage` has no field for both, so awaiting fails with
    /// `Error::InvalidArgument`.
    pub fn reply_to_ephemeral(mut self, ephemeral_message_id: i32) -> Self {
        self.reply_to = Some(ReplyTo::Ephemeral(ephemeral_message_id));
        self
    }

    /// Sends into this forum topic.
    ///
    /// `ephemeral.sendMessage` has no topic field, so without [`reply_to`](Self::reply_to) the message is sent as a
    /// reply to the topic's root message, which is how a topic travels.
    pub fn message_thread_id(mut self, thread_id: i32) -> Self {
        self.message_thread_id = Some(thread_id);
        self
    }

    /// Attaches a reply markup to the message.
    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
        self
    }

    async fn request(
        self,
        (chat_id, peer): (i64, base::InputPeer),
        receiver_id: base::InputUser,
        message: String,
        entities: Option<Vec<base::MessageEntity>>,
        rich_message: Option<base::InputRichMessage>,
    ) -> Result<functions::ephemeral::SendMessage> {
        // `ephemeral.sendMessage` has no top_msg_id: a topic travels only in inputReplyToMessage, so the reply is
        // written the way `messages.sendMessage` writes it.
        let thread = self.message_thread_id.filter(|&id| id != 1);
        let (reply, ephemeral) = match self.reply_to {
            Some(ReplyTo::Message(reply)) => (Some(reply), None),
            Some(ReplyTo::Ephemeral(id)) => (None, Some(id)),
            None => (None, None),
        };
        let reply_to = match ephemeral {
            Some(id) => {
                let reply = types::InputReplyToEphemeralMessage { id };
                Some(base::InputReplyTo::InputReplyToEphemeralMessage(reply))
            }
            None => {
                let options = SendOptions { reply_to: reply, message_thread_id: thread, ..SendOptions::default() };
                options.input_reply_to(self.client, chat_id, thread).await?
            }
        };
        let media = match &self.media {
            Some(media) => Some(self.client.input_media(&peer, media, None, None).await?),
            None => None,
        };
        Ok(functions::ephemeral::SendMessage {
            invert_media: self.media.as_ref().is_some_and(|m| m.show_caption_above_media),
            welcome: false,
            anchor: self.replace,
            noforwards: self.protect_content,
            peer: Some(peer),
            receiver_id,
            query_id: self.callback_query_id,
            message,
            entities,
            media,
            reply_markup: ReplyMarkup::write_option(self.reply_markup.as_ref(), self.client).await?,
            rich_message,
            random_id: i64::from_le_bytes(random()),
            reply_to,
        })
    }

    async fn send(mut self) -> Result<EphemeralMessage> {
        let client = self.client;
        // Only `Message::reply_ephemeral` builds a request without a receiver: its message had no sender user.
        let receiver = self.receiver_user_id.take().ok_or(Error::InvalidArgument("this message has no sender user"))?;
        // `anchor` says "replace the pressed message", which needs the press it answers.
        if self.replace && self.callback_query_id.is_none() {
            return Err(Error::InvalidArgument("replace_callback_query_message needs a callback_query_id"));
        }
        // A topic has no encoding next to a reply to an ephemeral message.
        if self.message_thread_id.filter(|&id| id != 1).is_some()
            && matches!(self.reply_to, Some(ReplyTo::Ephemeral(_)))
        {
            return Err(Error::InvalidArgument("a thread can't be named when replying to an ephemeral message"));
        }
        let text = std::mem::replace(&mut self.text, MessageText::Text(Text::new()));
        let (message, entities, rich_message) = match &mut self.media {
            Some(media) => {
                if matches!(&text, MessageText::Rich(_)) {
                    return Err(Error::InvalidArgument("a rich message can't carry media"));
                }
                if !matches!(&text, MessageText::Text(text) if text.is_empty()) {
                    return Err(Error::InvalidArgument("a media message takes its caption from InputMedia::caption"));
                }
                let (caption, entities) =
                    client.write_text(std::mem::take(&mut media.caption), media.parse_mode).await?;
                (Some(caption), (!entities.is_empty()).then_some(entities), None)
            }
            None => text.write(client, self.parse_mode).await?,
        };
        let chat = client.resolve(&self.chat_id).await?;
        let receiver_id = client.resolve_user(&receiver).await?;
        let request = self.request(chat, receiver_id, message.unwrap_or_default(), entities, rich_message).await?;
        EphemeralMessage::from_updates(client, client.invoke(&request).await?)
    }
}

caption_setters!(SendEphemeralMessage);
into_future!(SendEphemeralMessage, EphemeralMessage);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, types};
    use crate::types::{InputMedia, InputRichMessage, Message, Text};
    use crate::utils::get_peer;

    fn peer() -> base::InputPeer {
        base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id: 42, access_hash: 7 }))
    }

    fn receiver() -> base::InputUser {
        base::InputUser::InputUser(Box::new(types::InputUser { user_id: 1000, access_hash: 9 }))
    }

    fn seeded(client: &Client) {
        let mut peers = client.0.peers.lock().unwrap();
        for user_id in [7, 1000] {
            let user = types::InputPeerUser { user_id, access_hash: 1 };
            peers.insert(user_id, base::InputPeer::InputPeerUser(Box::new(user)));
        }
    }

    /// A message in the topic `thread`, which is its own root when its id is the thread id.
    fn topic_message(client: &Client, id: i32, thread: i32) -> Message {
        let raw = Box::new(Message::bare(id, get_peer(7), 0));
        let mut message = Message::parse(client, raw, &Entities::default());
        (message.message_thread_id, message.is_topic_message) = (Some(thread), true);
        message
    }

    fn topic_reply(reply_to_msg_id: i32, top_msg_id: Option<i32>) -> Vec<u8> {
        let reply = base::InputReplyTo::InputReplyToMessage(Box::new(types::InputReplyToMessage {
            reply_to_msg_id,
            top_msg_id,
            reply_to_peer_id: None,
            quote_text: None,
            quote_entities: None,
            quote_offset: None,
            monoforum_peer_id: None,
            todo_item_id: None,
            poll_option: None,
            quote_rich: None,
        }));
        reply.to_bytes()
    }

    /// The flags word of `ephemeral.sendMessage`, right after the constructor id.
    fn flags(body: &[u8]) -> u32 {
        u32::from_le_bytes(body[4..8].try_into().unwrap())
    }

    #[tokio::test]
    async fn maps_builder_to_request() {
        let client = Client::offline();
        let send = client.send_ephemeral_message(-1, 1000, "hi").callback_query_id(55).reply_to_ephemeral(3);
        let request = send.protect_content().request((-1, peer()), receiver(), "hi".into(), None, None).await.unwrap();
        assert_eq!(
            (request.peer, request.receiver_id, request.query_id, request.message.as_str(), request.noforwards),
            (Some(peer()), receiver(), Some(55), "hi", true)
        );
        let reply_to = types::InputReplyToEphemeralMessage { id: 3 };
        let reply_to = Some(base::InputReplyTo::InputReplyToEphemeralMessage(reply_to));
        assert_eq!((request.reply_to, request.rich_message, request.anchor), (reply_to, None, false));
    }

    #[tokio::test]
    async fn media_sends_refuse_message_text() {
        let (client, mut sent) = Client::recording();
        let send = client.send_ephemeral_message("me", "me", "text").media(InputMedia::photo("cat.jpg"));
        assert!(matches!(send.await, Err(Error::InvalidArgument(_))));
        let send = client.send_ephemeral_message("me", "me", InputRichMessage::html("<b>hi</b>"));
        assert!(matches!(send.media(InputMedia::photo("cat.jpg")).await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn built_text_keeps_its_entities() {
        let (client, mut sent) = Client::recording();
        let _ = client.send_ephemeral_message("me", "me", Text::new().bold("hi")).await;
        let body = sent.try_recv().expect("no request sent");
        let bold = vec![base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 2 })];
        let mut text = String::from("hi").to_bytes();
        text.extend(bold.to_bytes());
        assert!(body.windows(text.len()).any(|w| w == text));
    }

    #[tokio::test]
    async fn rich_text_goes_to_rich_message() {
        let (client, mut sent) = Client::recording();
        let _ = client.send_ephemeral_message("me", "me", InputRichMessage::html("<b>hi</b>")).await;
        let body = sent.try_recv().expect("no request sent");
        assert_ne!(flags(&body) & 1 << 4, 0, "rich_message is flags.4");
        let html = types::InputRichMessageHTML { rtl: false, noautolink: false, html: "<b>hi</b>".into(), files: None };
        let rich = base::InputRichMessage::InputRichMessageHTML(Box::new(html)).to_bytes();
        assert!(body.windows(rich.len()).any(|w| w == rich));
    }

    #[tokio::test]
    async fn replace_callback_query_message_sets_anchor() {
        let (client, mut sent) = Client::recording();
        let refused = client.send_ephemeral_message("me", "me", "hi").replace_callback_query_message().await;
        assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{refused:?}");
        assert!(sent.try_recv().is_err(), "a replace without a query must not send");

        let send = client.send_ephemeral_message("me", "me", "hi").callback_query_id(55);
        let _ = send.replace_callback_query_message().await;
        let body = sent.try_recv().expect("no request sent");
        assert_ne!(flags(&body) & 1 << 9, 0, "anchor is flags.9");
    }

    #[tokio::test]
    async fn a_reply_to_a_topic_message_carries_the_topic() {
        let (client, mut sent) = Client::recording();
        seeded(&client);
        let _ = topic_message(&client, 12, 3).reply_ephemeral("hi").await;
        let body = sent.try_recv().expect("no request sent");
        let reply = topic_reply(12, Some(3));
        assert!(body.windows(reply.len()).any(|w| w == reply), "the topic is not in the reply");
    }

    #[tokio::test]
    async fn a_reply_to_the_topic_root_carries_no_topic() {
        let (client, mut sent) = Client::recording();
        seeded(&client);
        let _ = topic_message(&client, 3, 3).reply_ephemeral("hi").await;
        let body = sent.try_recv().expect("no request sent");
        let reply = topic_reply(3, None);
        assert!(body.windows(reply.len()).any(|w| w == reply), "the root reply names a topic");
    }

    #[tokio::test]
    async fn a_thread_without_a_reply_replies_to_the_topic_root() {
        let (client, mut sent) = Client::recording();
        let _ = client.send_ephemeral_message("me", "me", "hi").message_thread_id(3).await;
        let body = sent.try_recv().expect("no request sent");
        let reply = topic_reply(3, None);
        assert!(body.windows(reply.len()).any(|w| w == reply), "the topic is not the reply target");
    }

    #[tokio::test]
    async fn a_thread_refuses_a_reply_to_an_ephemeral_message() {
        let (client, mut sent) = Client::recording();
        let send = client.send_ephemeral_message("me", "me", "hi").message_thread_id(3).reply_to_ephemeral(7);
        assert!(matches!(send.await, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
    }
}
