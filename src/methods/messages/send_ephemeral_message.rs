use super::send_options::caption_setters;
use crate::Client;
use crate::crypto::random;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{EphemeralMessage, PeerRef, ReplyMarkup, ReplyParameters};

enum ReplyTo {
    Message(ReplyParameters),
    Ephemeral(i32),
}

#[must_use = "a request does nothing until it is awaited"]
pub struct SendEphemeralMessage<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    receiver_user_id: PeerRef,
    text: String,
    parse_mode: Option<ParseMode>,
    protect_content: bool,
    callback_query_id: Option<i64>,
    reply_to: Option<ReplyTo>,
    reply_markup: Option<ReplyMarkup>,
}

impl Client {
    pub fn send_ephemeral_message(
        &self,
        chat_id: impl Into<PeerRef>,
        receiver_user_id: impl Into<PeerRef>,
        text: impl Into<String>,
    ) -> SendEphemeralMessage<'_> {
        SendEphemeralMessage {
            client: self,
            chat_id: chat_id.into(),
            receiver_user_id: receiver_user_id.into(),
            text: text.into(),
            parse_mode: None,
            protect_content: false,
            callback_query_id: None,
            reply_to: None,
            reply_markup: None,
        }
    }
}

impl SendEphemeralMessage<'_> {
    pub fn callback_query_id(mut self, query_id: i64) -> Self {
        self.callback_query_id = Some(query_id);
        self
    }

    pub fn protect_content(mut self) -> Self {
        self.protect_content = true;
        self
    }

    pub fn reply_to(mut self, reply: impl Into<ReplyParameters>) -> Self {
        self.reply_to = Some(ReplyTo::Message(reply.into()));
        self
    }

    pub fn reply_to_ephemeral(mut self, ephemeral_message_id: i32) -> Self {
        self.reply_to = Some(ReplyTo::Ephemeral(ephemeral_message_id));
        self
    }

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
    ) -> Result<functions::ephemeral::SendMessage> {
        let reply_to = match self.reply_to {
            Some(ReplyTo::Message(reply)) => {
                let reply = reply.write(self.client, chat_id).await?;
                Some(base::InputReplyTo::InputReplyToMessage(Box::new(reply)))
            }
            Some(ReplyTo::Ephemeral(id)) => {
                let reply = types::InputReplyToEphemeralMessage { id };
                Some(base::InputReplyTo::InputReplyToEphemeralMessage(Box::new(reply)))
            }
            None => None,
        };
        Ok(functions::ephemeral::SendMessage {
            invert_media: false,
            welcome: false,
            anchor: false,
            noforwards: self.protect_content,
            peer: Some(peer),
            receiver_id,
            query_id: self.callback_query_id,
            message,
            entities,
            media: None,
            reply_markup: self.reply_markup.as_ref().map(ReplyMarkup::write),
            rich_message: None,
            random_id: i64::from_le_bytes(random()),
            reply_to,
        })
    }

    async fn send(self) -> Result<EphemeralMessage> {
        let client = self.client;
        let chat = client.resolve(&self.chat_id).await?;
        let receiver_id = client.resolve_user(&self.receiver_user_id).await?;
        let (message, entities) = client.parse_text(&self.text, self.parse_mode).await?;
        let entities = (!entities.is_empty()).then_some(entities);
        let request = self.request(chat, receiver_id, message, entities).await?;
        EphemeralMessage::from_updates(client, client.invoke(&request).await?)
    }
}

caption_setters!(SendEphemeralMessage);
into_future!(SendEphemeralMessage, EphemeralMessage);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{base, types};

    fn peer() -> base::InputPeer {
        base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id: 42, access_hash: 7 }))
    }

    fn receiver() -> base::InputUser {
        base::InputUser::InputUser(Box::new(types::InputUser { user_id: 1000, access_hash: 9 }))
    }

    #[tokio::test]
    async fn maps_builder_to_request() {
        let client = Client::offline();
        let send = client.send_ephemeral_message(-1, 1000, "hi").callback_query_id(55).reply_to_ephemeral(3);
        let request = send.protect_content().request((-1, peer()), receiver(), "hi".into(), None).await.unwrap();
        assert_eq!(
            (request.peer, request.receiver_id, request.query_id, request.message.as_str(), request.noforwards),
            (Some(peer()), receiver(), Some(55), "hi", true)
        );
        let reply_to = types::InputReplyToEphemeralMessage { id: 3 };
        let reply_to = Some(base::InputReplyTo::InputReplyToEphemeralMessage(Box::new(reply_to)));
        assert_eq!((request.reply_to, request.rich_message), (reply_to, None));
    }
}
