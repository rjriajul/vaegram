use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Sticker;

pub struct SearchStickers<'a> {
    client: &'a Client,
    request: functions::messages::SearchStickers,
}

impl Client {
    pub fn search_stickers(&self, query: impl Into<String>) -> SearchStickers<'_> {
        let request = functions::messages::SearchStickers {
            emojis: false,
            q: query.into(),
            emoticon: String::new(),
            lang_code: Vec::new(),
            offset: 0,
            limit: 50,
            hash: 0,
        };
        SearchStickers { client: self, request }
    }
}

impl SearchStickers<'_> {
    pub fn custom_emoji(mut self) -> Self {
        self.request.emojis = true;
        self
    }

    pub fn emoji(mut self, emoji: impl Into<String>) -> Self {
        self.request.emoticon = emoji.into();
        self
    }

    pub fn lang_codes(mut self, lang_codes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.request.lang_code = lang_codes.into_iter().map(Into::into).collect();
        self
    }

    /// Skips this many results; pass the number already fetched to get the next page.
    pub fn offset(mut self, offset: i32) -> Self {
        self.request.offset = offset;
        self
    }

    pub fn limit(mut self, limit: i32) -> Self {
        self.request.limit = limit;
        self
    }
}

impl SearchStickers<'_> {
    async fn send(self) -> Result<Vec<Sticker>> {
        Ok(match self.client.invoke(&self.request).await? {
            base::messages::FoundStickers::FoundStickers(found) => {
                found.stickers.iter().filter_map(Sticker::parse).collect()
            }
            base::messages::FoundStickers::FoundStickersNotModified(_) => Vec::new(),
        })
    }
}

into_future!(SearchStickers, Vec<Sticker>);
