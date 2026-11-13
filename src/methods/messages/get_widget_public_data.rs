use crate::raw::functions;
use crate::types::{PeerRef, WidgetPublicData};
use crate::{Client, errors::Result};

/// Request built by [`Client::get_widget_public_data`].
#[must_use = "a request does nothing until it is awaited"]
pub struct GetWidgetPublicData<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    hash: i64,
}

impl Client {
    /// Fetches the current public data of the widget in message `message_id`, as an app polls a widget on screen.
    ///
    /// Awaiting the request returns [`WidgetPublicData`]; poll again after its `period` seconds.
    pub fn get_widget_public_data(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> GetWidgetPublicData<'_> {
        GetWidgetPublicData { client: self, chat_id: chat_id.into(), message_id, hash: 0 }
    }
}

impl GetWidgetPublicData<'_> {
    /// Passes the hash of the data you already have, such as [`Widget::public_data_hash`](crate::types::Widget),
    /// so Telegram answers [`WidgetPublicData::NotModified`] when it hasn't changed. Defaults to 0.
    pub fn hash(mut self, hash: i64) -> Self {
        self.hash = hash;
        self
    }

    async fn send(self) -> Result<WidgetPublicData> {
        let request = functions::messages::GetWidgetPublicData {
            peer: self.client.resolve_peer(self.chat_id).await?,
            msg_id: self.message_id,
            hash: self.hash,
        };
        Ok(WidgetPublicData::parse(self.client.invoke(&request).await?))
    }
}
into_future!(GetWidgetPublicData, WidgetPublicData);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::WidgetPublicData;

    #[tokio::test]
    async fn polls_with_hash_and_returns_public_data() {
        let reply = base::messages::WidgetPublicData::WidgetPublicData(Box::new(types::messages::WidgetPublicData {
            hash: 9,
            data: "hello".into(),
            period: 20,
        }));
        let (client, mut sent) = Client::scripted(vec![Ok(reply.to_bytes())]);
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 42, access_hash: 7 }));
        {
            client.0.peers.lock().unwrap().insert(42, peer.clone());
        }
        let result = client.get_widget_public_data(42, 11).hash(8).await.unwrap();
        assert_eq!(result, WidgetPublicData::Modified { hash: 9, data: "hello".into(), period: 20 });
        assert_eq!(
            sent.try_recv().unwrap(),
            functions::messages::GetWidgetPublicData { peer, msg_id: 11, hash: 8 }.to_bytes()
        );
    }
}
