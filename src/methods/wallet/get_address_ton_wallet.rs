use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::UserTonWalletAddress;

impl Client {
    /// `None` when Telegram knows no wallet at that address.
    pub async fn get_address_ton_wallet(&self, address: &str) -> Result<Option<UserTonWalletAddress>> {
        if address.is_empty() {
            return Err(Error::InvalidArgument("address is empty"));
        }
        let request =
            functions::wallet::GetUserAddresses { force: false, id: Vec::new(), addresses: vec![address.into()] };
        Ok(UserTonWalletAddress::parse_all(self, self.invoke(&request).await?).into_iter().next())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::functions::wallet::GetUserAddresses;
    use crate::raw::{Serializable, base, types};

    fn reply(addresses: Vec<base::WalletUserAddress>) -> Vec<u8> {
        let list = types::wallet::UserAddresses { addresses, users: Vec::new() };
        base::wallet::UserAddresses::UserAddresses(Box::new(list)).to_bytes()
    }

    #[tokio::test]
    async fn looks_up_the_wallet_at_an_address() {
        let address = types::WalletUserAddress { user_id: None, address: "EQx".into(), public_key: vec![1] };
        let (client, mut rx) =
            Client::scripted(vec![Ok(reply(vec![base::WalletUserAddress::WalletUserAddress(Box::new(address))]))]);
        let found = client.get_address_ton_wallet("EQx").await.unwrap().unwrap();
        assert_eq!((found.user, found.wallet_address.as_str(), found.public_key), (None, "EQx", vec![1]));
        let expected = GetUserAddresses { force: false, id: Vec::new(), addresses: vec!["EQx".into()] };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn an_unknown_address_gives_none() {
        // Telegram returned no wallet for the address.
        let (client, _rx) = Client::scripted(vec![Ok(reply(Vec::new()))]);
        assert_eq!(client.get_address_ton_wallet("EQx").await.unwrap(), None);
    }

    #[tokio::test]
    async fn an_empty_address_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        let result = client.get_address_ton_wallet("").await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn telegram_errors_come_back_unchanged() {
        let (client, _rx) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "WALLET_NOT_AVAILABLE")))]);
        let result = client.get_address_ton_wallet("EQx").await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "WALLET_NOT_AVAILABLE"), "{result:?}");
    }
}
