use crate::Client;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::Sticker;

/// Stickers matching a text query, built by [`Client::search_stickers`].
///
/// Awaiting it collects the stickers into a `Vec`, up to [`limit`](SearchStickers::limit);
/// [`next`](SearchStickers::next) reads them one at a time. Stickers are fetched 50 to a page.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct SearchStickers<'a> {
    client: &'a Client,
    request: functions::messages::SearchStickers,
    pages: OffsetPages<Sticker, i32>,
}

impl Client {
    /// Searches stickers by a text query.
    pub fn search_stickers(&self, query: impl Into<String>) -> SearchStickers<'_> {
        let request = functions::messages::SearchStickers {
            emojis: false,
            q: query.into(),
            emoticon: String::new(),
            lang_code: Vec::new(),
            offset: 0,
            limit: 0,
            hash: 0,
        };
        SearchStickers { client: self, request, pages: OffsetPages::starting_at(50, 0) }
    }
}

impl SearchStickers<'_> {
    /// Searches custom emoji instead of stickers.
    pub fn custom_emoji(mut self) -> Self {
        self.request.emojis = true;
        self
    }

    /// Narrows the results to stickers for this emoji.
    pub fn emoji(mut self, emoji: impl Into<String>) -> Self {
        self.request.emoticon = emoji.into();
        self
    }

    /// Limits the search to these language codes.
    pub fn lang_codes(mut self, lang_codes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.request.lang_code = lang_codes.into_iter().map(Into::into).collect();
        self
    }

    /// Skips this many results.
    pub fn offset(mut self, offset: i32) -> Self {
        self.pages.set_offset(offset);
        self
    }

    /// Stops after this many stickers in total. The default reads every match.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Returns the next sticker, or `None` once the matches or the limit run out.
    pub async fn next(&mut self) -> Result<Option<Sticker>> {
        loop {
            if let Some(sticker) = self.pages.pop() {
                return Ok(Some(sticker));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            self.request.offset = self.pages.offset().copied().unwrap_or(0);
            self.request.limit = limit;
            match self.client.invoke(&self.request).await? {
                base::messages::FoundStickers::FoundStickers(found) => {
                    // An empty page ends the list too: asking again would loop forever.
                    let next = found.next_offset.filter(|_| !found.stickers.is_empty());
                    self.pages.push(found.stickers.iter().filter_map(Sticker::parse).collect(), next);
                }
                base::messages::FoundStickers::FoundStickersNotModified(_) => self.pages.push(Vec::new(), None),
            }
        }
    }
}

pager!(SearchStickers, Sticker);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    fn found(next_offset: Option<i32>, ids: &[i64]) -> crate::Result<Vec<u8>> {
        let stickers = ids.iter().map(|&id| base::Document::DocumentEmpty(types::DocumentEmpty { id })).collect();
        let found = types::messages::FoundStickers { next_offset, hash: 0, stickers };
        Ok(base::messages::FoundStickers::FoundStickers(Box::new(found)).to_bytes())
    }

    fn request(offset: i32, limit: i32) -> Vec<u8> {
        let request = functions::messages::SearchStickers {
            emojis: false,
            q: "cat".into(),
            emoticon: String::new(),
            lang_code: Vec::new(),
            offset,
            limit,
            hash: 0,
        };
        request.to_bytes()
    }

    #[tokio::test]
    async fn follows_the_next_offset_until_telegram_gives_none() {
        let (client, mut sent) = Client::scripted(vec![found(Some(60), &[1, 2]), found(None, &[3])]);
        assert!(client.search_stickers("cat").offset(10).await.unwrap().is_empty());
        assert_eq!(sent.try_recv().unwrap(), request(10, 50));
        assert_eq!(sent.try_recv().unwrap(), request(60, 50));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn an_empty_page_ends_the_search_and_the_limit_sizes_the_page() {
        let (client, mut sent) = Client::scripted(vec![found(Some(5), &[]), found(None, &[1])]);
        assert!(client.search_stickers("cat").await.unwrap().is_empty());
        assert_eq!(sent.try_recv().unwrap(), request(0, 50));
        assert!(client.search_stickers("cat").limit(1).await.unwrap().is_empty());
        assert_eq!(sent.try_recv().unwrap(), request(0, 1));
        assert!(sent.try_recv().is_err());
    }
}
