use crate::Client;
use crate::crypto::random;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{Message, PeerRef, ReplyParameters};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendInlineBotResult<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    query_id: i64,
    result_id: String,
    reply_to: Option<ReplyParameters>,
    disable_notification: bool,
    hide_via: bool,
}

impl Client {
    pub fn send_inline_bot_result(
        &self,
        chat_id: impl Into<PeerRef>,
        query_id: i64,
        result_id: impl Into<String>,
    ) -> SendInlineBotResult<'_> {
        SendInlineBotResult {
            client: self,
            chat_id: chat_id.into(),
            query_id,
            result_id: result_id.into(),
            reply_to: None,
            disable_notification: false,
            hide_via: false,
        }
    }
}

impl SendInlineBotResult<'_> {
    pub fn reply_to(mut self, reply: impl Into<ReplyParameters>) -> Self {
        self.reply_to = Some(reply.into());
        self
    }

    pub fn disable_notification(mut self) -> Self {
        self.disable_notification = true;
        self
    }

    pub fn hide_via(mut self) -> Self {
        self.hide_via = true;
        self
    }

    async fn send(self) -> Result<Message> {
        let client = self.client;
        let chat_id = client.peer_id(self.chat_id).await?;
        let reply_to = match &self.reply_to {
            Some(reply) => Some(base::InputReplyTo::InputReplyToMessage(Box::new(reply.write(client, chat_id).await?))),
            None => None,
        };
        let request = functions::messages::SendInlineBotResult {
            silent: self.disable_notification,
            background: false,
            clear_draft: false,
            hide_via: self.hide_via,
            peer: client.resolve_peer(chat_id).await?,
            reply_to,
            random_id: i64::from_le_bytes(random()),
            query_id: self.query_id,
            id: self.result_id,
            schedule_date: None,
            send_as: None,
            quick_reply_shortcut: None,
            allow_paid_stars: None,
        };
        let updates = client.invoke(&request).await?;
        Message::from_updates(client, updates, chat_id, (String::new(), None))
    }
}

into_future!(SendInlineBotResult, Message);
