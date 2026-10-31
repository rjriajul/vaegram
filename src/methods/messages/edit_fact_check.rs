use super::send_options::caption_setters;
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{Message, PeerRef, Text};

#[must_use = "a request does nothing until it is awaited"]
pub struct EditFactCheck<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    text: Text,
    parse_mode: Option<ParseMode>,
}

impl Client {
    /// Only accounts Telegram made fact-checkers may call this.
    pub fn edit_fact_check(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        text: impl Into<Text>,
    ) -> EditFactCheck<'_> {
        EditFactCheck { client: self, chat_id: chat_id.into(), message_id, text: text.into(), parse_mode: None }
    }
}

impl EditFactCheck<'_> {
    async fn send(self) -> Result<Message> {
        let client = self.client;
        let (chat_id, peer) = client.resolve(&self.chat_id).await?;
        let (text, entities) = client.write_text(self.text, self.parse_mode).await?;
        let text = base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text, entities }));
        let updates =
            client.invoke(&functions::messages::EditFactCheck { peer, msg_id: self.message_id, text }).await?;
        Message::from_updates(client, updates, chat_id, (String::new(), None))
    }
}

caption_setters!(EditFactCheck);
into_future!(EditFactCheck, Message);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::Message;
    use crate::utils::{get_channel_id, get_peer};

    fn channel() -> base::InputPeer {
        base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id: 9, access_hash: 1 }))
    }

    #[tokio::test]
    async fn edits_the_fact_check_and_returns_the_message() {
        let message = base::Message::Message(Box::new(Message::bare(5, get_peer(get_channel_id(9)), 1)));
        let update = types::UpdateEditChannelMessage { message, pts: 1, pts_count: 1 };
        let updates = types::Updates {
            updates: vec![base::Update::UpdateEditChannelMessage(Box::new(update))],
            users: Vec::new(),
            chats: Vec::new(),
            date: 1,
            seq: 0,
        };
        let (client, mut rx) = Client::scripted(vec![Ok(base::Updates::Updates(Box::new(updates)).to_bytes())]);
        client.0.peers.lock().unwrap().insert(get_channel_id(9), channel());

        let edited = client.edit_fact_check(get_channel_id(9), 5, "<b>Fake</b>").html().await.unwrap();
        assert_eq!(edited.id, 5);
        let bold = base::MessageEntity::MessageEntityBold(types::MessageEntityBold { offset: 0, length: 4 });
        let text = types::TextWithEntities { text: "Fake".into(), entities: vec![bold] };
        let text = base::TextWithEntities::TextWithEntities(Box::new(text));
        let expected = functions::messages::EditFactCheck { peer: channel(), msg_id: 5, text };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn fact_check_errors_pass_through() {
        let (client, _rx) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(403, "CHAT_ADMIN_REQUIRED")))]);
        client.0.peers.lock().unwrap().insert(get_channel_id(9), channel());
        let result = client.edit_fact_check(get_channel_id(9), 5, "Fake").await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "CHAT_ADMIN_REQUIRED"), "{result:?}");
    }
}
