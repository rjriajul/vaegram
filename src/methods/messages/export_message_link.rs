use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct ExportMessageLink<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    grouped: bool,
    thread: bool,
}

impl Client {
    /// A `t.me` link to a message in a channel or supergroup; other chats have no message links.
    pub fn export_message_link(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> ExportMessageLink<'_> {
        ExportMessageLink { client: self, chat_id: chat_id.into(), message_id, grouped: false, thread: false }
    }
}

impl ExportMessageLink<'_> {
    pub fn grouped(mut self) -> Self {
        self.grouped = true;
        self
    }

    pub fn thread(mut self) -> Self {
        self.thread = true;
        self
    }

    async fn send(self) -> Result<String> {
        let request = functions::channels::ExportMessageLink {
            grouped: self.grouped,
            thread: self.thread,
            channel: self.client.resolve_channel(&self.chat_id).await?,
            id: self.message_id,
        };
        let base::ExportedMessageLink::ExportedMessageLink(link) = self.client.invoke(&request).await?;
        Ok(link.link)
    }
}

into_future!(ExportMessageLink, String);

#[cfg(test)]
mod tests {
    use crate::client::sent;
    use crate::raw::functions::channels;
    use crate::raw::{Identifiable, base, types};
    use crate::utils::get_channel_id;
    use crate::{Client, Error};

    #[tokio::test]
    async fn only_channels_have_links() {
        let (client, mut rx) = Client::recording();
        let channel = get_channel_id(9);
        let peer = types::InputPeerChannel { channel_id: 9, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(channel, base::InputPeer::InputPeerChannel(Box::new(peer)));

        let group = client.export_message_link(-5, 1).await;
        assert!(matches!(group, Err(Error::InvalidArgument(_))));
        let channel = sent(&mut rx, client.export_message_link(channel, 1)).await;
        assert_eq!(channel, channels::ExportMessageLink::CONSTRUCTOR_ID);
    }
}
