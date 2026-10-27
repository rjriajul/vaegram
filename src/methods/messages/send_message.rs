use std::future::{Future, IntoFuture};
use std::pin::Pin;

use crate::Client;
use crate::crypto::random;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::parser;
use crate::raw::{base, functions, types};
use crate::types::{Message, ReplyMarkup};

pub struct SendMessage<'a> {
    client: &'a Client,
    chat_id: i64,
    text: String,
    parse_mode: ParseMode,
    reply_to: Option<i32>,
    reply_markup: Option<ReplyMarkup>,
}

impl Client {
    pub fn send_message(&self, chat_id: i64, text: impl Into<String>) -> SendMessage<'_> {
        SendMessage {
            client: self,
            chat_id,
            text: text.into(),
            parse_mode: ParseMode::Disabled,
            reply_to: None,
            reply_markup: None,
        }
    }

    pub(crate) async fn parse_text(&self, text: &str, mode: ParseMode) -> Result<(String, Vec<base::MessageEntity>)> {
        let (text, mut entities) = parser::parse(text, mode);
        for entity in &mut entities {
            if let base::MessageEntity::InputMessageEntityMentionName(m) = entity {
                if let base::InputUser::InputUser(u) = &mut m.user_id {
                    if let base::InputPeer::InputPeerUser(p) = self.resolve_peer(u.user_id).await? {
                        u.access_hash = p.access_hash;
                    }
                }
            }
        }
        Ok((text, entities))
    }
}

impl SendMessage<'_> {
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

    pub fn reply_to(mut self, message_id: i32) -> Self {
        self.reply_to = Some(message_id);
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
        let reply_to = self.reply_to.map(|reply_to_msg_id| {
            base::InputReplyTo::InputReplyToMessage(Box::new(types::InputReplyToMessage {
                reply_to_msg_id,
                top_msg_id: None,
                reply_to_peer_id: None,
                quote_text: None,
                quote_entities: None,
                quote_offset: None,
                monoforum_peer_id: None,
                todo_item_id: None,
                poll_option: None,
                quote_rich: None,
            }))
        });
        let request = functions::messages::SendMessage {
            no_webpage: false,
            silent: false,
            background: false,
            clear_draft: false,
            noforwards: false,
            update_stickersets_order: false,
            invert_media: false,
            allow_paid_floodskip: false,
            peer,
            reply_to,
            message: message.clone(),
            // Telegram drops a second message with the same random_id, so retries can't double-send.
            random_id: i64::from_le_bytes(random()),
            reply_markup: self.reply_markup.as_ref().map(ReplyMarkup::write),
            entities: entities.clone(),
            schedule_date: None,
            schedule_repeat_period: None,
            send_as: None,
            quick_reply_shortcut: None,
            effect: None,
            allow_paid_stars: None,
            suggested_post: None,
            rich_message: None,
        };
        let updates = client.invoke(&request).await?;
        Message::from_updates(client, updates, self.chat_id, (message, entities))
    }
}

impl<'a> IntoFuture for SendMessage<'a> {
    type Output = Result<Message>;
    type IntoFuture = Pin<Box<dyn Future<Output = Self::Output> + Send + 'a>>;

    fn into_future(self) -> Self::IntoFuture {
        Box::pin(self.send())
    }
}
