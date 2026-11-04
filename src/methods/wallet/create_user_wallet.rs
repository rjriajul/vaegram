use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{PeerRef, UserTonWalletAddress};

impl Client {
    /// Asks Telegram to create the user's wallet; `None` when it returns no address. This changes the user's
    /// account state, so call it only when you mean to.
    pub async fn create_user_wallet(&self, user_id: impl Into<PeerRef>) -> Result<Option<UserTonWalletAddress>> {
        let id = vec![self.resolve_user(&user_id.into()).await?];
        let request = functions::wallet::GetUserAddresses { force: true, id, addresses: Vec::new() };
        Ok(UserTonWalletAddress::parse_all(self, self.invoke(&request).await?).into_iter().next())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::wallet::GetUserAddresses;
    use crate::raw::{Serializable, base, types};

    #[tokio::test]
    async fn create_forces_the_address() {
        let address = types::WalletUserAddress { user_id: None, address: "EQx".into(), public_key: vec![1] };
        let list = types::wallet::UserAddresses {
            addresses: vec![base::WalletUserAddress::WalletUserAddress(Box::new(address))],
            users: Vec::new(),
        };
        let reply = base::wallet::UserAddresses::UserAddresses(Box::new(list)).to_bytes();
        let (client, mut rx) = Client::scripted(vec![Ok(reply)]);
        let peer = types::InputPeerUser { user_id: 7, access_hash: 9 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(peer)));
        let found = client.create_user_wallet(7).await.unwrap().unwrap();
        assert_eq!(found.wallet_address, "EQx");
        let sent = GetUserAddresses {
            force: true,
            id: vec![base::InputUser::InputUser(Box::new(types::InputUser { user_id: 7, access_hash: 9 }))],
            addresses: Vec::new(),
        };
        assert_eq!(rx.try_recv().unwrap(), sent.to_bytes());
    }
}
