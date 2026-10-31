use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

impl Client {
    pub async fn get_suggested_sticker_set_name(&self, title: &str) -> Result<String> {
        let request = functions::stickers::SuggestShortName { title: title.into() };
        let base::stickers::SuggestedShortName::SuggestedShortName(name) = self.invoke(&request).await?;
        Ok(name.short_name)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::functions::stickers::SuggestShortName;
    use crate::raw::{Serializable, base, types};

    #[tokio::test]
    async fn returns_telegrams_suggestion() {
        let name = types::stickers::SuggestedShortName { short_name: "my_set".into() };
        let reply = base::stickers::SuggestedShortName::SuggestedShortName(Box::new(name)).to_bytes();
        let (client, mut rx) = Client::scripted(vec![Ok(reply)]);
        assert_eq!(client.get_suggested_sticker_set_name("My Set").await.unwrap(), "my_set");
        assert_eq!(rx.try_recv().unwrap(), SuggestShortName { title: "My Set".into() }.to_bytes());
    }

    #[tokio::test]
    async fn telegram_errors_come_back_unchanged() {
        let (client, _rx) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(400, "TITLE_INVALID")))]);
        let result = client.get_suggested_sticker_set_name("").await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "TITLE_INVALID"), "{result:?}");
    }
}
