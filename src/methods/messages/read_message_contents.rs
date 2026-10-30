use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::session::Affected;
use crate::types::PeerRef;
use crate::utils::CHANNEL_OFFSET;

impl Client {
    /// Marks messages as opened: voice and video notes as played, mentions as seen, and self-destructing media as
    /// viewed, which starts its timer.
    pub async fn read_message_contents(&self, chat_id: impl Into<PeerRef>, message_ids: &[i32]) -> Result<()> {
        if message_ids.is_empty() {
            return Err(Error::InvalidArgument("read_message_contents takes at least one message id"));
        }
        let chat_id = self.peer_id(chat_id.into()).await?;
        let id = message_ids.to_vec();
        if chat_id <= -CHANNEL_OFFSET {
            let channel = self.resolve_channel(&chat_id.into()).await?;
            self.invoke(&functions::channels::ReadMessageContents { channel, id }).await?;
        } else {
            let request = functions::messages::ReadMessageContents { id };
            let base::messages::AffectedMessages::AffectedMessages(a) = self.invoke(&request).await?;
            self.0.events.affected(Affected { channel_id: None, pts: a.pts, pts_count: a.pts_count });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::client::sent;
    use crate::errors::Error;
    use crate::raw::functions::{channels, messages};
    use crate::raw::{Identifiable, base, types};
    use crate::utils::get_channel_id;

    #[tokio::test]
    async fn channels_use_channel_methods() {
        let (client, mut rx) = Client::recording();
        let channel = get_channel_id(9);
        let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(channel, base::InputPeer::InputPeerChannel(Box::new(peer)));

        let group = sent(&mut rx, client.read_message_contents(-5, &[1])).await;
        let channel = sent(&mut rx, client.read_message_contents(channel, &[1])).await;
        let expected = (messages::ReadMessageContents::CONSTRUCTOR_ID, channels::ReadMessageContents::CONSTRUCTOR_ID);
        assert_eq!((group, channel), expected);
    }

    #[tokio::test]
    async fn no_ids_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        assert!(matches!(client.read_message_contents(-5, &[]).await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}
