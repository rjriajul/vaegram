use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Asks Telegram whether a sticker set can be created with this short name (the part after `t.me/addstickers/`).
    ///
    /// Returns `true` when it's free. Telegram answers a name it won't take with an error such as
    /// `SHORT_NAME_OCCUPIED` or `SHORT_NAME_INVALID`, which comes back unchanged.
    pub async fn check_sticker_set_name(&self, short_name: &str) -> Result<bool> {
        self.invoke(&functions::stickers::CheckShortName { short_name: short_name.into() }).await
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::Serializable;
    use crate::raw::functions::stickers::CheckShortName;

    #[tokio::test]
    async fn asks_about_the_short_name() {
        let (client, mut rx) = Client::scripted_raw(vec![Ok(true.to_bytes())]);
        assert!(client.check_sticker_set_name("my_set").await.unwrap());
        assert_eq!(rx.try_recv().unwrap(), CheckShortName { short_name: "my_set".into() }.to_bytes());
    }

    #[tokio::test]
    async fn telegram_errors_come_back_unchanged() {
        let (client, _rx) = Client::scripted_raw(vec![Err(Error::Rpc(RpcError::parse(400, "SHORT_NAME_OCCUPIED")))]);
        let result = client.check_sticker_set_name("taken").await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "SHORT_NAME_OCCUPIED"), "{result:?}");
    }
}
