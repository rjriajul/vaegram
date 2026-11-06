use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Read the current value from [`Client::get_active_sessions`].
    pub async fn set_inactive_session_ttl(&self, days: i32) -> Result<()> {
        self.invoke(&functions::account::SetAuthorizationTTL { authorization_ttl_days: days }).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::Serializable;
    use crate::raw::functions::account;

    #[tokio::test]
    async fn sends_the_days() {
        let (client, mut rx) = Client::scripted(vec![Ok(true.to_bytes())]);
        client.set_inactive_session_ttl(30).await.unwrap();
        assert_eq!(rx.try_recv().unwrap(), account::SetAuthorizationTTL { authorization_ttl_days: 30 }.to_bytes());
    }
}
