use crate::Client;
use crate::errors::Result;
use crate::methods::messages::{SendOptions, send_builder};
use crate::raw::{base, types};
use crate::types::{Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct ForwardStory<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    from_chat_id: PeerRef,
    story_id: i32,
    options: SendOptions,
}

impl Client {
    pub fn forward_story(
        &self,
        chat_id: impl Into<PeerRef>,
        from_chat_id: impl Into<PeerRef>,
        story_id: i32,
    ) -> ForwardStory<'_> {
        ForwardStory {
            client: self,
            chat_id: chat_id.into(),
            from_chat_id: from_chat_id.into(),
            story_id,
            options: SendOptions::default(),
        }
    }
}

impl ForwardStory<'_> {
    async fn send(self) -> Result<Message> {
        let peer = self.client.resolve_peer(self.from_chat_id).await?;
        let media = base::InputMedia::InputMediaStory(Box::new(types::InputMediaStory { peer, id: self.story_id }));
        self.client.send_media(self.chat_id, media, "", None, self.options).await
    }
}

send_builder!(ForwardStory);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Identifiable, Serializable, base, functions, types};
    use crate::utils::get_channel_id;

    fn known_channel(client: &Client) -> i64 {
        let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(get_channel_id(9), base::InputPeer::InputPeerChannel(Box::new(peer)));
        get_channel_id(9)
    }

    #[tokio::test]
    async fn sends_the_story_as_media() {
        let (client, mut rx) = Client::recording();
        let channel = known_channel(&client);
        let _ = client.forward_story("me", channel, 5).disable_notification().await;
        let body = rx.try_recv().unwrap();
        assert_eq!(u32::from_le_bytes(body[..4].try_into().unwrap()), functions::messages::SendMedia::CONSTRUCTOR_ID);
        let flags = u32::from_le_bytes(body[4..8].try_into().unwrap());
        assert_ne!(flags & (1 << 5), 0, "silent flag missing");
        let peer =
            base::InputPeer::InputPeerChannel(Box::new(types::InputPeerChannel { channel_id: 9, access_hash: 1 }));
        let media = base::InputMedia::InputMediaStory(Box::new(types::InputMediaStory { peer, id: 5 })).to_bytes();
        assert!(body.windows(media.len()).any(|w| w == media.as_slice()));
    }

    #[tokio::test]
    async fn telegram_errors_come_back_unchanged() {
        let (client, _rx) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "STORY_ID_INVALID")))]);
        let result = client.forward_story("me", "me", 5).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "STORY_ID_INVALID"), "{result:?}");
    }
}
