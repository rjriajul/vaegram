use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;

impl Client {
    /// Sets the auto-delete timer new chats of the account start with. 0 turns it off.
    ///
    /// [`set_chat_ttl`](Client::set_chat_ttl) sets the timer of one existing chat.
    pub async fn set_default_history_ttl(&self, ttl_seconds: i32) -> Result<()> {
        accepted(self.invoke(&functions::messages::SetDefaultHistoryTTL { period: ttl_seconds }).await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Serializable, functions};

    #[tokio::test]
    async fn sends_the_period() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        client.set_default_history_ttl(86400).await.unwrap();
        let expected = functions::messages::SetDefaultHistoryTTL { period: 86400 };
        assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn telegram_errors_come_back_unchanged() {
        let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::parse(400, "TTL_PERIOD_INVALID")))]);
        let result = client.set_default_history_ttl(5).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "TTL_PERIOD_INVALID"), "{result:?}");
    }
}
