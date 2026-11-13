use crate::Client;
use crate::errors::Result;
use crate::types::UniqueGift;

impl Client {
    /// Returns a unique gift by its name.
    ///
    /// Same call as [`Client::get_upgraded_gift`], which is what Bot API 10.4 named `getUniqueGift`; both take the
    /// gift's name (`PlushPepe-3`) or its `t.me/nft/<name>` link. Fails with `Error::Unexpected` if the name belongs to
    /// a regular gift.
    pub async fn get_unique_gift(&self, name: &str) -> Result<UniqueGift> {
        self.get_upgraded_gift(name).await
    }
}

#[cfg(test)]
mod tests {
    use super::super::craft_gift::tests::unique;
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    fn reply() -> Vec<u8> {
        let reply = types::payments::UniqueStarGift { gift: unique(), chats: Vec::new(), users: Vec::new() };
        base::payments::UniqueStarGift::UniqueStarGift(Box::new(reply)).to_bytes()
    }

    #[tokio::test]
    async fn asks_for_the_gift_by_its_name() {
        let (client, mut sent) = Client::scripted(vec![Ok(reply())]);
        let gift = client.get_unique_gift("PlushPepe-3").await.unwrap();
        assert_eq!((gift.name.as_str(), gift.number), ("PlushPepe-3", 3));
        let request = functions::payments::GetUniqueStarGift { slug: "PlushPepe-3".into() };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
    }
}
