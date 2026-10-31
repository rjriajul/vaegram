use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{EphemeralMessage, PeerRef};

impl Client {
    pub async fn get_welcome_messages(&self, chat_id: impl Into<PeerRef>) -> Result<Vec<EphemeralMessage>> {
        let request = functions::ephemeral::GetWelcomeMessages { peer: self.resolve_peer(chat_id).await?, hash: 0 };
        Ok(match self.invoke(&request).await? {
            base::ephemeral::WelcomeMessages::WelcomeMessages(list) => {
                let entities = Entities::default();
                list.messages.into_iter().map(|m| EphemeralMessage::parse(self, m, &entities)).collect()
            }
            base::ephemeral::WelcomeMessages::WelcomeMessagesNotModified => Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::ephemeral::GetWelcomeMessages;
    use crate::raw::{Serializable, base, types};
    use crate::types::messages_and_media::ephemeral_message::tests::{channel, ephemeral};
    use crate::utils::get_channel_id;

    fn request() -> Vec<u8> {
        GetWelcomeMessages { peer: base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 }), hash: 0 }
            .to_bytes()
    }

    #[tokio::test]
    async fn returns_the_stored_welcome_messages() {
        let list = types::ephemeral::WelcomeMessages { hash: 1, messages: vec![ephemeral(false, Some(channel()))] };
        let reply = base::ephemeral::WelcomeMessages::WelcomeMessages(Box::new(list)).to_bytes();
        let (client, mut rx) = Client::scripted(vec![Ok(reply)]);
        let messages = client.get_welcome_messages(-5_i64).await.unwrap();
        assert_eq!(rx.try_recv().unwrap(), request());
        assert_eq!(messages.len(), 1);
        let m = &messages[0];
        assert_eq!((m.id, m.text.as_str(), m.chat.id), (7, "secret", get_channel_id(42)));
    }

    #[tokio::test]
    async fn not_modified_means_no_messages() {
        // Hash 0 never matches, so this reply only comes from an odd server; there's nothing to return.
        let reply = base::ephemeral::WelcomeMessages::WelcomeMessagesNotModified.to_bytes();
        let (client, mut rx) = Client::scripted(vec![Ok(reply)]);
        assert!(client.get_welcome_messages(-5_i64).await.unwrap().is_empty());
        assert_eq!(rx.try_recv().unwrap(), request());
    }
}
