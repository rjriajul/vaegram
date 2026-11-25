use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    /// Turns the translation offer of one chat on or off for this account.
    ///
    /// With `enabled` set to `false`, Telegram stops offering to translate the chat's messages.
    pub async fn toggle_peer_translations(&self, chat_id: impl Into<PeerRef>, enabled: bool) -> Result<()> {
        let peer = self.resolve_peer(chat_id).await?;
        accepted(self.invoke(&functions::messages::TogglePeerTranslations { disabled: !enabled, peer }).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn disabled_is_the_opposite_of_enabled() {
        let (client, mut sent) = Client::scripted_raw(vec![Ok(true.to_bytes()), Ok(true.to_bytes())]);
        client.toggle_peer_translations(-5_i64, false).await.unwrap();
        client.toggle_peer_translations(-5_i64, true).await.unwrap();
        let peer = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 });
        for disabled in [true, false] {
            let expected = functions::messages::TogglePeerTranslations { disabled, peer: peer.clone() };
            assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
        }
    }
}
