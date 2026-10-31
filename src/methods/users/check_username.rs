use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;

impl Client {
    pub async fn check_username(&self, chat_id: impl Into<PeerRef>, username: impl Into<String>) -> Result<bool> {
        let chat = chat_id.into();
        let username = username.into();
        if let (_, base::InputPeer::InputPeerSelf) = self.resolve(&chat).await? {
            self.invoke(&functions::account::CheckUsername { username }).await
        } else {
            let channel = self.resolve_channel(&chat).await?;
            self.invoke(&functions::channels::CheckUsername { channel, username }).await
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, Result, RpcError};
    use crate::raw::{Serializable, base, functions, types};
    use crate::utils::get_channel_id;

    fn seeded(replies: Vec<Result<Vec<u8>>>) -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (client, sent) = Client::scripted(replies);
        let mut peers = client.0.peers.lock().unwrap();
        let user = types::InputPeerUser { user_id: 7, access_hash: 2 };
        peers.insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        let channel = types::InputPeerChannel { channel_id: 9, access_hash: 3 };
        peers.insert(get_channel_id(9), base::InputPeer::InputPeerChannel(Box::new(channel)));
        drop(peers);
        (client, sent)
    }

    #[tokio::test]
    async fn the_account_and_a_channel_use_their_own_checks() {
        let (client, mut sent) = seeded(vec![Ok(true.to_bytes()), Ok(false.to_bytes())]);
        assert!(client.check_username("me", "name").await.unwrap());
        assert_eq!(
            sent.recv().await.unwrap(),
            functions::account::CheckUsername { username: "name".into() }.to_bytes()
        );

        // A taken name is the server's `false`, passed on as is.
        assert!(!client.check_username(get_channel_id(9), "name").await.unwrap());
        let channel = base::InputChannel::InputChannel(Box::new(types::InputChannel { channel_id: 9, access_hash: 3 }));
        let expected = functions::channels::CheckUsername { channel, username: "name".into() };
        assert_eq!(sent.recv().await.unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn groups_and_users_are_refused_before_sending() {
        let (client, mut sent) = seeded(Vec::new());
        for chat in [-5, 7] {
            let result = client.check_username(chat, "name").await;
            assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        }
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn errors_come_back_unchanged() {
        let (client, _sent) = seeded(vec![Err(Error::Rpc(RpcError::new(400, "USERNAME_OCCUPIED")))]);
        let result = client.check_username("me", "name").await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.is("USERNAME_OCCUPIED")), "{result:?}");
    }
}
