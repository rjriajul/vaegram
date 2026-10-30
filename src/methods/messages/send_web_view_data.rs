use crate::Client;
use crate::crypto::random;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn send_web_view_data(&self, bot_id: impl Into<PeerRef>, button_text: &str, data: &str) -> Result<()> {
        if button_text.is_empty() || data.is_empty() {
            return Err(Error::InvalidArgument("send_web_view_data takes a button text and data"));
        }
        let request = functions::messages::SendWebViewData {
            bot: self.resolve_user(&bot_id.into()).await?,
            // Telegram drops a repeated random_id, so a retried call can't deliver the data twice.
            random_id: i64::from_le_bytes(random()),
            button_text: button_text.into(),
            data: data.into(),
        };
        self.invoke(&request).await.map(drop)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{base, types};

    #[tokio::test]
    async fn an_empty_button_or_data_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        let peer = types::InputPeerUser { user_id: 42, access_hash: 7 };
        client.0.peers.lock().unwrap().insert(42, base::InputPeer::InputPeerUser(Box::new(peer)));
        for (button, data) in [("", "x"), ("Go", "")] {
            assert!(matches!(client.send_web_view_data(42, button, data).await, Err(Error::InvalidArgument(_))));
        }
        assert!(rx.try_recv().is_err());
    }
}
