use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Logs out one device by its session id.
    pub async fn terminate_session(&self, session_id: i64) -> Result<bool> {
        self.invoke(&functions::account::ResetAuthorization { hash: session_id }).await
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, functions};

    #[tokio::test]
    async fn sends_the_session_hash() {
        let (client, mut sent) = Client::recording();
        let _ = client.terminate_session(-77).await;
        assert_eq!(sent.recv().await.unwrap(), functions::account::ResetAuthorization { hash: -77 }.to_bytes());
    }
}
