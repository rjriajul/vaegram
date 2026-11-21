use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};

impl Client {
    /// Searches custom emoji by an emoji, returning their document ids.
    ///
    /// [`get_custom_emoji_stickers`](Client::get_custom_emoji_stickers) turns the ids into stickers.
    pub async fn search_custom_emoji(&self, emoji: impl Into<String>) -> Result<Vec<i64>> {
        let request = functions::messages::SearchCustomEmoji { emoticon: emoji.into(), hash: 0 };
        Ok(match self.invoke(&request).await? {
            base::EmojiList::EmojiList(list) => list.document_id,
            base::EmojiList::EmojiListNotModified => Vec::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn lists_the_ids_and_an_unchanged_answer_is_empty() {
        let found = base::EmojiList::EmojiList(Box::new(types::EmojiList { hash: 2, document_id: vec![7, 8] }));
        let unchanged = base::EmojiList::EmojiListNotModified;
        let (client, mut sent) = Client::scripted(vec![Ok(found.to_bytes()), Ok(unchanged.to_bytes())]);
        assert_eq!(client.search_custom_emoji("🐱").await.unwrap(), [7, 8]);
        assert!(client.search_custom_emoji("🐱").await.unwrap().is_empty());
        let expected = functions::messages::SearchCustomEmoji { emoticon: "🐱".into(), hash: 0 };
        for _ in 0..2 {
            assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
        }
    }
}
