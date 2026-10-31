use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

impl Client {
    pub async fn suggest_birthday(
        &self,
        user_id: impl Into<PeerRef>,
        day: i32,
        month: i32,
        year: Option<i32>,
    ) -> Result<()> {
        let id = self.resolve_user(&user_id.into()).await?;
        let birthday = base::Birthday::Birthday(Box::new(types::Birthday { day, month, year }));
        self.invoke(&functions::users::SuggestBirthday { id, birthday }).await?;
        Ok(())
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

    fn no_updates() -> Vec<u8> {
        let updates = types::Updates { updates: Vec::new(), users: Vec::new(), chats: Vec::new(), date: 0, seq: 0 };
        base::Updates::Updates(Box::new(updates)).to_bytes()
    }

    fn request(year: Option<i32>) -> Vec<u8> {
        let id = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 7, access_hash: 2 }));
        let birthday = base::Birthday::Birthday(Box::new(types::Birthday { day: 31, month: 10, year }));
        functions::users::SuggestBirthday { id, birthday }.to_bytes()
    }

    #[tokio::test]
    async fn sends_the_date_with_or_without_a_year() {
        let (client, mut sent) = seeded(vec![Ok(no_updates()), Ok(no_updates())]);
        client.suggest_birthday(7, 31, 10, Some(1990)).await.unwrap();
        assert_eq!(sent.recv().await.unwrap(), request(Some(1990)));
        client.suggest_birthday(7, 31, 10, None).await.unwrap();
        assert_eq!(sent.recv().await.unwrap(), request(None));
    }

    #[tokio::test]
    async fn errors_come_back_unchanged() {
        let (client, _sent) = seeded(vec![Err(Error::Rpc(RpcError::new(400, "BIRTHDAY_ALREADY")))]);
        let result = client.suggest_birthday(7, 31, 10, None).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.is("BIRTHDAY_ALREADY")), "{result:?}");
    }

    #[tokio::test]
    async fn a_channel_is_refused_before_sending() {
        let (client, mut sent) = seeded(Vec::new());
        let result = client.suggest_birthday(get_channel_id(9), 31, 10, None).await;
        assert!(matches!(result, Err(Error::PeerNotFound(_))), "{result:?}");
        assert!(sent.try_recv().is_err());
    }
}
