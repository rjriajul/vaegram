use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Turns the display of sensitive content (adult media) on or off for the account.
    ///
    /// Telegram refuses it with an error when the account may not change the setting, for instance in a region
    /// where the choice is locked.
    pub async fn set_content_settings(&self, sensitive_enabled: bool) -> Result<()> {
        accepted(self.invoke(&functions::account::SetContentSettings { sensitive_enabled }).await?)
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
        for sensitive_enabled in [true, false] {
            client.set_content_settings(sensitive_enabled).await.unwrap();
            let request = account::SetContentSettings { sensitive_enabled };
            assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
        }
    }
}
