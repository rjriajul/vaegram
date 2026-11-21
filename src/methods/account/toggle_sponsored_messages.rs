use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Turns sponsored messages (ads in channels) on or off for the account.
    ///
    /// Only Telegram Premium accounts can turn them off; Telegram answers with an error for others.
    pub async fn toggle_sponsored_messages(&self, enabled: bool) -> Result<()> {
        accepted(self.invoke(&functions::account::ToggleSponsoredMessages { enabled }).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::Serializable;
    use crate::raw::functions::account;

    #[tokio::test]
    async fn sends_the_flag_both_ways() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes()), Ok(true.to_bytes())]);
        for enabled in [false, true] {
            client.toggle_sponsored_messages(enabled).await.unwrap();
            assert_eq!(sent.try_recv().unwrap(), account::ToggleSponsoredMessages { enabled }.to_bytes());
        }
    }
}
