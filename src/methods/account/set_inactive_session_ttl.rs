use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Sets how many days an unused session lives before Telegram ends it.
    ///
    /// Read the current value from [`Client::get_active_sessions`].
    pub async fn set_inactive_session_ttl(&self, days: i32) -> Result<()> {
        accepted(self.invoke(&functions::account::SetAuthorizationTTL { authorization_ttl_days: days }).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::Serializable;
    use crate::raw::functions::account;

    #[tokio::test]
    async fn sends_the_days() {
        let (client, mut rx) = Client::scripted(vec![Ok(true.to_bytes())]);
        client.set_inactive_session_ttl(30).await.unwrap();
        assert_eq!(rx.try_recv().unwrap(), account::SetAuthorizationTTL { authorization_ttl_days: 30 }.to_bytes());
    }

    #[tokio::test]
    async fn a_false_answer_is_an_error() {
        let (client, _rx) = Client::scripted(vec![Ok(false.to_bytes())]);
        let result = client.set_inactive_session_ttl(30).await;
        assert!(matches!(result, Err(Error::Unexpected(_))), "{result:?}");
    }
}
