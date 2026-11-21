use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::Sticker;

/// The most custom emoji ids Telegram takes in one `messages.getCustomEmojiDocuments` call.
const MAX_IDS_PER_CALL: usize = 200;

impl Client {
    /// Returns the stickers of the given custom emoji ids.
    ///
    /// Telegram takes 200 ids per call, so longer lists go in several calls and the stickers come back in the
    /// order of the ids. Ids Telegram doesn't know are skipped.
    pub async fn get_custom_emoji_stickers(&self, custom_emoji_ids: &[i64]) -> Result<Vec<Sticker>> {
        let mut stickers = Vec::with_capacity(custom_emoji_ids.len());
        for chunk in custom_emoji_ids.chunks(MAX_IDS_PER_CALL) {
            let request = functions::messages::GetCustomEmojiDocuments { document_id: chunk.to_vec() };
            stickers.extend(self.invoke(&request).await?.iter().filter_map(Sticker::parse));
        }
        Ok(stickers)
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, functions};

    #[tokio::test]
    async fn long_lists_go_in_chunks_of_200() {
        let empty = || Ok(Vec::<crate::raw::base::Document>::new().to_bytes());
        let (client, mut rx) = Client::scripted((0..3).map(|_| empty()).collect());
        let ids: Vec<i64> = (1..=450).collect();
        assert!(client.get_custom_emoji_stickers(&ids).await.unwrap().is_empty());
        for chunk in ids.chunks(200) {
            let expected = functions::messages::GetCustomEmojiDocuments { document_id: chunk.to_vec() };
            assert_eq!(rx.try_recv().unwrap(), expected.to_bytes());
        }
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn no_ids_send_nothing() {
        let (client, mut rx) = Client::recording();
        assert!(client.get_custom_emoji_stickers(&[]).await.unwrap().is_empty());
        assert!(rx.try_recv().is_err());
    }
}
