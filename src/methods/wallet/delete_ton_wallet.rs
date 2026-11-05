use super::replace_ton_wallet::password_check;
use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::TonWalletState;

#[must_use = "a request does nothing until it is awaited"]
/// Request built by [`Client::delete_ton_wallet`].
pub struct DeleteTonWallet<'a> {
    client: &'a Client,
    password: Option<String>,
}

impl Client {
    /// Detaches the account's wallet, starts a fresh one and returns the new [`TonWalletState`].
    ///
    /// The old wallet can't be attached again.
    pub fn delete_ton_wallet(&self) -> DeleteTonWallet<'_> {
        DeleteTonWallet { client: self, password: None }
    }
}

impl DeleteTonWallet<'_> {
    /// Confirms the change with the account's cloud password.
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.password = Some(password.into());
        self
    }

    async fn send(self) -> Result<TonWalletState> {
        let password = password_check(self.client, self.password.as_deref()).await?;
        let request =
            functions::wallet::ReplaceWallet { wallet: base::InputWalletReplacement::InputWalletNew, password };
        Ok(TonWalletState::parse(self.client, &self.client.invoke(&request).await?))
    }
}

into_future!(DeleteTonWallet, TonWalletState);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::functions::wallet::ReplaceWallet;
    use crate::raw::{Serializable, base, types};

    #[tokio::test]
    async fn delete_sends_new_wallet() {
        let reply = base::WalletState::WalletStateEmpty(types::WalletStateEmpty { creating: false }).to_bytes();
        let (client, mut rx) = Client::scripted(vec![Ok(reply)]);
        client.delete_ton_wallet().await.unwrap();
        let expected = ReplaceWallet { wallet: base::InputWalletReplacement::InputWalletNew, password: None };
        assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn without_password_one_request() {
        let reply = base::WalletState::WalletStateEmpty(types::WalletStateEmpty { creating: false }).to_bytes();
        let (client, mut rx) = Client::scripted(vec![Ok(reply)]);
        client.delete_ton_wallet().await.unwrap();
        rx.try_recv().unwrap();
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_password_is_fetched_then_sent_with_the_new_wallet() {
        use crate::methods::wallet::replace_ton_wallet::tests::{password_reply, sends_password};
        let state = base::WalletState::WalletStateEmpty(types::WalletStateEmpty { creating: false }).to_bytes();
        let (client, mut rx) = Client::scripted(vec![Ok(password_reply()), Ok(state)]);
        client.delete_ton_wallet().password("hunter2").await.unwrap();
        rx.try_recv().unwrap();
        assert!(sends_password(&rx.try_recv().unwrap()));
        assert!(rx.try_recv().is_err());
    }
}
