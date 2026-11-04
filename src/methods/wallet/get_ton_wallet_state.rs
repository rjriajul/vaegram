use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::TonWalletState;

impl Client {
    /// Telegram answers `WALLET_UNAVAILABLE` where wallets aren't available to the account.
    pub async fn get_ton_wallet_state(&self) -> Result<TonWalletState> {
        Ok(TonWalletState::parse(self, &self.invoke(&functions::wallet::GetState {}).await?))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Serializable, base, types};

    #[tokio::test]
    async fn reads_the_state() {
        let state = base::WalletState::WalletStateEmpty(types::WalletStateEmpty { creating: true });
        let (client, _rx) = Client::scripted(vec![Ok(state.to_bytes())]);
        assert!(client.get_ton_wallet_state().await.unwrap().is_being_created);
    }

    #[tokio::test]
    async fn wallet_unavailable_is_returned_as_rpc() {
        let (client, _rx) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "WALLET_UNAVAILABLE")))]);
        let result = client.get_ton_wallet_state().await;
        assert!(matches!(&result, Err(e) if e.is("WALLET_UNAVAILABLE")), "{result:?}");
    }
}
