use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};

#[must_use = "a request does nothing until it is awaited"]
pub struct SetEmojiStatus<'a> {
    client: &'a Client,
    custom_emoji_id: Option<i64>,
    collectible_id: Option<i64>,
    until: Option<i32>,
}

impl Client {
    pub fn set_emoji_status(&self, custom_emoji_id: Option<i64>) -> SetEmojiStatus<'_> {
        SetEmojiStatus { client: self, custom_emoji_id, collectible_id: None, until: None }
    }
}

impl SetEmojiStatus<'_> {
    pub fn until(mut self, date: i32) -> Self {
        self.until = Some(date);
        self
    }

    /// Shows an owned unique gift as the status instead of a custom emoji: pass [`UniqueGift::id`] here and `None`
    /// to `set_emoji_status`.
    ///
    /// [`UniqueGift::id`]: crate::types::UniqueGift::id
    pub fn collectible_id(mut self, collectible_id: i64) -> Self {
        self.collectible_id = Some(collectible_id);
        self
    }

    async fn send(self) -> Result<bool> {
        let until = self.until;
        let emoji_status = match (self.custom_emoji_id, self.collectible_id) {
            (Some(_), Some(_)) => {
                return Err(Error::InvalidArgument("an emoji status is a custom emoji or a collectible, not both"));
            }
            (Some(document_id), None) => {
                base::EmojiStatus::EmojiStatus(Box::new(types::EmojiStatus { document_id, until }))
            }
            (None, Some(collectible_id)) => {
                base::EmojiStatus::InputEmojiStatusCollectible(Box::new(types::InputEmojiStatusCollectible {
                    collectible_id,
                    until,
                }))
            }
            (None, None) => base::EmojiStatus::EmojiStatusEmpty,
        };
        self.client.invoke(&functions::account::UpdateEmojiStatus { emoji_status }).await
    }
}

into_future!(SetEmojiStatus, bool);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn collectibles_become_the_status() {
        let (client, mut sent) = Client::recording();
        assert!(client.set_emoji_status(None).collectible_id(4).until(9).await.is_err());
        let collectible = types::InputEmojiStatusCollectible { collectible_id: 4, until: Some(9) };
        let emoji_status = base::EmojiStatus::InputEmojiStatusCollectible(Box::new(collectible));
        assert_eq!(sent.try_recv().unwrap(), functions::account::UpdateEmojiStatus { emoji_status }.to_bytes());
    }

    #[tokio::test]
    async fn an_emoji_and_a_collectible_together_are_refused_without_a_request() {
        let (client, mut sent) = Client::recording();
        let both = client.set_emoji_status(Some(1)).collectible_id(4).await;
        assert!(matches!(both, Err(Error::InvalidArgument(_))));
        assert!(sent.try_recv().is_err());
    }
}
