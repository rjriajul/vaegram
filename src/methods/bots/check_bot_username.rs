use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn check_bot_username(&self, username: impl Into<String>) -> Result<bool> {
        self.invoke(&functions::bots::CheckUsername { additional: false, username: username.into() }).await
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Serializable, functions};

    #[tokio::test]
    async fn asks_about_a_main_bot_username() {
        let (client, mut sent) = Client::scripted(vec![Ok(true.to_bytes())]);
        assert!(client.check_bot_username("name_bot").await.unwrap());
        let expected = functions::bots::CheckUsername { additional: false, username: "name_bot".into() };
        assert_eq!(sent.recv().await.unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn errors_come_back_unchanged() {
        let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "USERNAME_SUFFIX_MISSING")))]);
        let result = client.check_bot_username("name").await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.is("USERNAME_SUFFIX_MISSING")), "{result:?}");
    }
}
