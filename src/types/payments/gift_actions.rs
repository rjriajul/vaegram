use crate::Client;
use crate::methods::SetEmojiStatus;
use crate::types::UniqueGift;

impl UniqueGift {
    /// Shows this gift as the account's emoji status; the account must own it.
    pub fn wear<'a>(&self, client: &'a Client) -> SetEmojiStatus<'a> {
        client.set_emoji_status(None).collectible_id(self.id)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::UniqueGift;

    #[tokio::test]
    async fn wear_sets_the_collectible_status() {
        let gift = UniqueGift {
            id: 11,
            gift_id: 2,
            base_name: "Plush Pepe".into(),
            name: "PlushPepe-3".into(),
            number: 3,
            model: None,
            symbol: None,
            backdrop: None,
            is_premium: false,
            is_burned: false,
            publisher_chat: None,
        };
        let (client, mut sent) = Client::recording();
        assert!(gift.wear(&client).await.is_err());
        let collectible = types::InputEmojiStatusCollectible { collectible_id: 11, until: None };
        let emoji_status = base::EmojiStatus::InputEmojiStatusCollectible(Box::new(collectible));
        assert_eq!(sent.try_recv().unwrap(), functions::account::UpdateEmojiStatus { emoji_status }.to_bytes());
    }
}
