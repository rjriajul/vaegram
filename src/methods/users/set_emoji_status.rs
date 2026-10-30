use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};

#[must_use = "a request does nothing until it is awaited"]
pub struct SetEmojiStatus<'a> {
    client: &'a Client,
    custom_emoji_id: Option<i64>,
    until: Option<i32>,
}

impl Client {
    pub fn set_emoji_status(&self, custom_emoji_id: Option<i64>) -> SetEmojiStatus<'_> {
        SetEmojiStatus { client: self, custom_emoji_id, until: None }
    }
}

impl SetEmojiStatus<'_> {
    pub fn until(mut self, date: i32) -> Self {
        self.until = Some(date);
        self
    }

    async fn send(self) -> Result<bool> {
        let emoji_status = match self.custom_emoji_id {
            Some(document_id) => {
                base::EmojiStatus::EmojiStatus(Box::new(types::EmojiStatus { document_id, until: self.until }))
            }
            None => base::EmojiStatus::EmojiStatusEmpty,
        };
        self.client.invoke(&functions::account::UpdateEmojiStatus { emoji_status }).await
    }
}

into_future!(SetEmojiStatus, bool);
