use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Hides the bar at the top of a chat that offers to report it, block the other side or add them as a contact.
    ///
    /// [`get_chat_settings`](Client::get_chat_settings) tells which of those the bar offers.
    pub async fn hide_peer_settings_bar(&self, chat_id: impl Into<PeerRef>) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        accepted(self.invoke(&functions::messages::HidePeerSettingsBar { peer }).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn hides_the_bar_of_the_chat() {
        let (client, mut sent) = Client::scripted_raw(vec![Ok(true.to_bytes())]);
        client.hide_peer_settings_bar(-5_i64).await.unwrap();
        let peer = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 });
        assert_eq!(sent.try_recv().unwrap(), functions::messages::HidePeerSettingsBar { peer }.to_bytes());
    }
}
