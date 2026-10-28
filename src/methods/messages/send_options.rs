use crate::Client;
use crate::crypto::random;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{Message, PeerRef, ReplyMarkup, ReplyParameters, RichMessage, SuggestedPostParameters};

#[derive(Default)]
pub(crate) struct SendOptions {
    pub(crate) reply_to: Option<ReplyParameters>,
    pub(crate) message_thread_id: Option<i32>,
    pub(crate) direct_messages_topic_id: Option<PeerRef>,
    pub(crate) reply_markup: Option<ReplyMarkup>,
    pub(crate) business_connection_id: Option<String>,
    pub(crate) disable_notification: bool,
    pub(crate) protect_content: bool,
    pub(crate) allow_paid_broadcast: bool,
    pub(crate) paid_message_star_count: Option<i64>,
    pub(crate) message_effect_id: Option<i64>,
    pub(crate) suggested_post_parameters: Option<SuggestedPostParameters>,
    pub(crate) schedule_date: Option<i32>,
    pub(crate) no_webpage: bool,
    pub(crate) invert_media: bool,
}

macro_rules! send_builder {
    ($builder:ident) => {
        send_builder!($builder, $crate::types::Message);

        impl $builder<'_> {
            pub fn reply_markup(mut self, markup: impl Into<$crate::types::ReplyMarkup>) -> Self {
                self.options.reply_markup = Some(markup.into());
                self
            }

            pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
                self.options.business_connection_id = Some(id.into());
                self
            }
        }

        impl $crate::methods::OnBehalf for $builder<'_> {
            fn business_connection(&mut self) -> &mut Option<String> {
                &mut self.options.business_connection_id
            }
        }
    };
    ($builder:ident, $output:ty) => {
        impl $builder<'_> {
            pub fn reply_to(mut self, reply: impl Into<$crate::types::ReplyParameters>) -> Self {
                self.options.reply_to = Some(reply.into());
                self
            }

            pub fn message_thread_id(mut self, thread_id: i32) -> Self {
                self.options.message_thread_id = Some(thread_id);
                self
            }

            pub fn direct_messages_topic_id(mut self, user_id: impl Into<$crate::types::PeerRef>) -> Self {
                self.options.direct_messages_topic_id = Some(user_id.into());
                self
            }

            pub fn disable_notification(mut self) -> Self {
                self.options.disable_notification = true;
                self
            }

            pub fn protect_content(mut self) -> Self {
                self.options.protect_content = true;
                self
            }

            /// Pays Stars to go over the broadcast limit of 30 messages a second.
            pub fn allow_paid_broadcast(mut self) -> Self {
                self.options.allow_paid_broadcast = true;
                self
            }

            pub fn paid_message_star_count(mut self, stars: i64) -> Self {
                self.options.paid_message_star_count = Some(stars);
                self
            }

            pub fn message_effect_id(mut self, effect_id: i64) -> Self {
                self.options.message_effect_id = Some(effect_id);
                self
            }

            pub fn suggested_post_parameters(mut self, parameters: $crate::types::SuggestedPostParameters) -> Self {
                self.options.suggested_post_parameters = Some(parameters);
                self
            }

            pub fn schedule_date(mut self, date: i32) -> Self {
                self.options.schedule_date = Some(date);
                self
            }
        }

        into_future!($builder, $output);
    };
}
pub(crate) use send_builder;

macro_rules! caption_setters {
    ($builder:ident) => {
        impl $builder<'_> {
            pub fn html(self) -> Self {
                self.parse_mode($crate::enums::ParseMode::Html)
            }

            pub fn markdown(self) -> Self {
                self.parse_mode($crate::enums::ParseMode::Markdown)
            }

            pub fn parse_mode(mut self, mode: $crate::enums::ParseMode) -> Self {
                self.parse_mode = Some(mode);
                self
            }
        }
    };
}
pub(crate) use caption_setters;

impl SendOptions {
    /// The forum topic to name. Per messages.sendMessage, the General topic (id 1) never is.
    pub(crate) fn thread(&self) -> Option<i32> {
        self.message_thread_id.filter(|&id| id != 1)
    }

    pub(crate) async fn input_reply_to(
        &self,
        client: &Client,
        chat_id: i64,
        thread: Option<i32>,
    ) -> Result<Option<base::InputReplyTo>> {
        let monoforum_peer_id = match &self.direct_messages_topic_id {
            Some(user) => Some(client.resolve(user).await?.1),
            None => None,
        };
        let mut reply = match (&self.reply_to, thread) {
            (Some(reply), _) => reply.write(client, chat_id).await?,
            (None, Some(thread)) => ReplyParameters::new(thread).write(client, chat_id).await?,
            (None, None) => {
                let Some(monoforum_peer_id) = monoforum_peer_id else { return Ok(None) };
                let reply = types::InputReplyToMonoForum { monoforum_peer_id };
                return Ok(Some(base::InputReplyTo::InputReplyToMonoForum(Box::new(reply))));
            }
        };
        // top_msg_id is set only when replying to another message in the topic.
        reply.top_msg_id = thread.filter(|&id| id != reply.reply_to_msg_id);
        reply.monoforum_peer_id = monoforum_peer_id;
        Ok(Some(base::InputReplyTo::InputReplyToMessage(Box::new(reply))))
    }
}

impl Client {
    pub(crate) async fn send_media(
        &self,
        chat_id: impl Into<PeerRef>,
        media: base::InputMedia,
        caption: &str,
        parse_mode: Option<ParseMode>,
        options: SendOptions,
    ) -> Result<Message> {
        let (message, entities) = self.parse_text(caption, parse_mode).await?;
        let text = (message, (!entities.is_empty()).then_some(entities));
        self.send_parsed(chat_id, Some(media), text, None, options).await
    }

    pub(crate) async fn send_parsed(
        &self,
        chat_id: impl Into<PeerRef>,
        media: Option<base::InputMedia>,
        (message, entities): (String, Option<Vec<base::MessageEntity>>),
        rich_message: Option<base::InputRichMessage>,
        options: SendOptions,
    ) -> Result<Message> {
        let (chat_id, peer) = self.resolve(&chat_id.into()).await?;
        let reply_to = options.input_reply_to(self, chat_id, options.thread()).await?;
        // Telegram drops a second message with the same random_id, so retries can't double-send.
        let random_id = i64::from_le_bytes(random());
        let reply_markup = options.reply_markup.as_ref().map(ReplyMarkup::write);
        let suggested_post = options.suggested_post_parameters.map(SuggestedPostParameters::write);
        let connection_id = options.business_connection_id;
        let (updates, sent, rich_message) = match media {
            Some(media) => {
                let request = functions::messages::SendMedia {
                    silent: options.disable_notification,
                    background: false,
                    clear_draft: false,
                    noforwards: options.protect_content,
                    update_stickersets_order: false,
                    invert_media: options.invert_media,
                    allow_paid_floodskip: options.allow_paid_broadcast,
                    peer,
                    reply_to,
                    media,
                    message,
                    random_id,
                    reply_markup,
                    entities,
                    schedule_date: options.schedule_date,
                    schedule_repeat_period: None,
                    send_as: None,
                    quick_reply_shortcut: None,
                    effect: options.message_effect_id,
                    allow_paid_stars: options.paid_message_star_count,
                    suggested_post,
                };
                let (updates, request) = self.invoke_business(connection_id.as_deref(), request).await?;
                (updates, (request.message, request.entities), None)
            }
            None => {
                let request = functions::messages::SendMessage {
                    no_webpage: options.no_webpage,
                    silent: options.disable_notification,
                    background: false,
                    clear_draft: false,
                    noforwards: options.protect_content,
                    update_stickersets_order: false,
                    invert_media: options.invert_media,
                    allow_paid_floodskip: options.allow_paid_broadcast,
                    peer,
                    reply_to,
                    message,
                    random_id,
                    reply_markup,
                    entities,
                    schedule_date: options.schedule_date,
                    schedule_repeat_period: None,
                    send_as: None,
                    quick_reply_shortcut: None,
                    effect: options.message_effect_id,
                    allow_paid_stars: options.paid_message_star_count,
                    suggested_post,
                    rich_message,
                };
                let (updates, request) = self.invoke_business(connection_id.as_deref(), request).await?;
                (updates, (request.message, request.entities), request.rich_message)
            }
        };
        let Some(rich_message) = rich_message else { return Message::from_updates(self, updates, chat_id, sent) };
        // A short "sent" reply has no rich message, so it is rebuilt from the request, minus photos and documents.
        let (text, sent_rich) = RichMessage::sent(rich_message);
        let mut message = Message::from_updates(self, updates, chat_id, (text, None))?;
        if message.rich_message.is_none() {
            message.rich_message = sent_rich;
        }
        Ok(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GROUP: i64 = -5;

    fn message(reply: Option<base::InputReplyTo>) -> types::InputReplyToMessage {
        match reply {
            Some(base::InputReplyTo::InputReplyToMessage(m)) => *m,
            other => panic!("not a message reply: {other:?}"),
        }
    }

    async fn written(client: &Client, options: SendOptions) -> Option<base::InputReplyTo> {
        options.input_reply_to(client, GROUP, options.thread()).await.unwrap()
    }

    async fn topic(client: &Client, reply: Option<i32>, thread: Option<i32>) -> Option<(i32, Option<i32>)> {
        let reply_to = reply.map(ReplyParameters::new);
        let options = SendOptions { reply_to, message_thread_id: thread, ..Default::default() };
        let m = message(Some(written(client, options).await?));
        Some((m.reply_to_msg_id, m.top_msg_id))
    }

    #[tokio::test]
    async fn topics_follow_the_send_message_rules() {
        let client = &Client::offline();
        assert_eq!(topic(client, None, None).await, None);
        assert_eq!(topic(client, None, Some(1)).await, None);
        assert_eq!(topic(client, None, Some(7)).await, Some((7, None)));
        assert_eq!(topic(client, Some(9), Some(7)).await, Some((9, Some(7))));
        assert_eq!(topic(client, Some(7), Some(7)).await, Some((7, None)));
        assert_eq!(topic(client, Some(9), Some(1)).await, Some((9, None)));
    }

    #[tokio::test]
    async fn reply_parameters_fill_quote_task_and_other_chat() {
        let client = &Client::offline();
        let reply = ReplyParameters::new(4).chat_id(-6).quote("<b>hi</b>").quote_position(2).checklist_task_id(3);
        let m = message(written(client, SendOptions { reply_to: Some(reply), ..Default::default() }).await);
        let other_chat = base::InputPeer::InputPeerChat(Box::new(types::InputPeerChat { chat_id: 6 }));
        assert_eq!((m.reply_to_msg_id, m.quote_offset, m.todo_item_id), (4, Some(2), Some(3)));
        assert_eq!((m.quote_text.as_deref(), m.reply_to_peer_id), (Some("<b>hi</b>"), Some(other_chat)));

        let reply = ReplyParameters::new(4).chat_id(GROUP).quote("<b>hi</b>").quote_parse_mode(ParseMode::Html);
        let m = message(written(client, SendOptions { reply_to: Some(reply), ..Default::default() }).await);
        assert_eq!((m.quote_text.as_deref(), m.reply_to_peer_id), (Some("hi"), None));
        assert_eq!(m.quote_entities.map(|e| e.len()), Some(1));
    }

    #[tokio::test]
    async fn unset_parse_modes_use_the_client_default() {
        let mut client = Client::offline();
        std::sync::Arc::get_mut(&mut client.0).unwrap().parse_mode = ParseMode::Html;
        let (text, entities) = client.parse_text("<b>hi</b>", None).await.unwrap();
        assert_eq!((text.as_str(), entities.len()), ("hi", 1));
        let (text, entities) = client.parse_text("<b>hi</b>", Some(ParseMode::Disabled)).await.unwrap();
        assert_eq!((text.as_str(), entities.len()), ("<b>hi</b>", 0));

        let reply = Some(ReplyParameters::new(4).quote("<b>hi</b>"));
        let m = message(written(&client, SendOptions { reply_to: reply, ..Default::default() }).await);
        assert_eq!(m.quote_text.as_deref(), Some("hi"));
    }

    #[tokio::test]
    async fn direct_messages_topics_name_the_user() {
        let client = &Client::offline();
        let user = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 42, access_hash: 1 }));
        client.0.peers.lock().unwrap().insert(42, user.clone());

        let options = SendOptions { direct_messages_topic_id: Some(42.into()), ..Default::default() };
        let topic = types::InputReplyToMonoForum { monoforum_peer_id: user.clone() };
        let expected = base::InputReplyTo::InputReplyToMonoForum(Box::new(topic));
        assert_eq!(written(client, options).await, Some(expected));

        let reply_to = Some(ReplyParameters::new(3));
        let options = SendOptions { reply_to, direct_messages_topic_id: Some(42.into()), ..Default::default() };
        let m = message(written(client, options).await);
        assert_eq!((m.reply_to_msg_id, m.monoforum_peer_id), (3, Some(user)));
    }
}
