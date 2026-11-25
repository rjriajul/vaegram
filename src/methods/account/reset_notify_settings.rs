use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Resets the notification settings of every chat, and the account-wide defaults, to Telegram's defaults.
    ///
    /// This drops every mute and every other exception at once.
    pub async fn reset_notify_settings(&self) -> Result<()> {
        accepted(self.invoke(&functions::account::ResetNotifySettings {}).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::Serializable;
    use crate::raw::functions::account;

    #[tokio::test]
    async fn sends_the_reset_and_checks_the_answer() {
        let (client, mut sent) = Client::scripted_raw(vec![Ok(true.to_bytes()), Ok(false.to_bytes())]);
        client.reset_notify_settings().await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), account::ResetNotifySettings {}.to_bytes());
        assert!(matches!(client.reset_notify_settings().await, Err(Error::Unexpected(_))));
    }
}
