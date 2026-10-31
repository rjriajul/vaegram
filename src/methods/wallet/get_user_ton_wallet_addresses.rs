use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::{PeerRef, UserTonWalletAddress};

impl Client {
    pub async fn get_user_ton_wallet_addresses(
        &self,
        user_ids: impl IntoIterator<Item = impl Into<PeerRef>>,
    ) -> Result<Vec<UserTonWalletAddress>> {
        let id = self.resolve_users(user_ids).await?;
        if id.is_empty() {
            return Err(Error::InvalidArgument("pass at least one user id"));
        }
        let request = functions::wallet::GetUserAddresses { force: false, id, addresses: Vec::new() };
        Ok(UserTonWalletAddress::parse_all(self, self.invoke(&request).await?))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::functions::wallet::GetUserAddresses;
    use crate::raw::{Serializable, base, types};

    fn reply(addresses: Vec<base::WalletUserAddress>, users: Vec<base::User>) -> Vec<u8> {
        base::wallet::UserAddresses::UserAddresses(Box::new(types::wallet::UserAddresses { addresses, users }))
            .to_bytes()
    }

    fn address(user_id: Option<i64>) -> base::WalletUserAddress {
        let address = types::WalletUserAddress { user_id, address: "EQx".into(), public_key: vec![1, 2] };
        base::WalletUserAddress::WalletUserAddress(Box::new(address))
    }

    fn named_user(id: i64) -> base::User {
        let mut buf = Vec::new();
        // Only access_hash and first_name are set.
        for word in [0b11u32, 0] {
            word.serialize(&mut buf);
        }
        id.serialize(&mut buf);
        (id * 10).serialize(&mut buf);
        "Ann".to_owned().serialize(&mut buf);
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    fn known_user(client: &Client) {
        let user = types::InputPeerUser { user_id: 7, access_hash: 2 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
    }

    #[tokio::test]
    async fn looks_up_the_users_wallets() {
        let replies = vec![Ok(reply(vec![address(Some(7)), address(None)], vec![named_user(7)]))];
        let (client, mut rx) = Client::scripted(replies);
        known_user(&client);
        let found = client.get_user_ton_wallet_addresses([7_i64]).await.unwrap();
        let user = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 7, access_hash: 2 }));
        let expected = GetUserAddresses { force: false, id: vec![user], addresses: Vec::new() };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());

        assert_eq!(found.len(), 2);
        let named = found[0].user.as_ref().map(|u| (u.id, u.first_name.as_str()));
        assert_eq!(
            (named, found[0].wallet_address.as_str(), found[0].public_key.as_slice()),
            (Some((7, "Ann")), "EQx", &[1, 2][..])
        );
        assert_eq!(found[1].user, None);
    }

    #[tokio::test]
    async fn no_wallet_means_an_empty_list() {
        // The user has no wallet, so Telegram lists no address.
        let (client, _rx) = Client::scripted(vec![Ok(reply(Vec::new(), Vec::new()))]);
        known_user(&client);
        assert!(client.get_user_ton_wallet_addresses([7_i64]).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn no_user_ids_are_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        let result = client.get_user_ton_wallet_addresses(Vec::<i64>::new()).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn telegram_errors_come_back_unchanged() {
        let (client, _rx) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "WALLET_NOT_AVAILABLE")))]);
        known_user(&client);
        let result = client.get_user_ton_wallet_addresses([7_i64]).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "WALLET_NOT_AVAILABLE"), "{result:?}");
    }
}
