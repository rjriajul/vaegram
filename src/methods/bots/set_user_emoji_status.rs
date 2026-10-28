use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct SetUserEmojiStatus<'a> {
    client: &'a Client,
    user_id: PeerRef,
    custom_emoji_id: Option<i64>,
    expiration_date: Option<i32>,
}

impl Client {
    /// Sets the emoji status of a user who allowed the bot to; without a custom emoji it clears the status.
    pub fn set_user_emoji_status(&self, user_id: impl Into<PeerRef>) -> SetUserEmojiStatus<'_> {
        SetUserEmojiStatus { client: self, user_id: user_id.into(), custom_emoji_id: None, expiration_date: None }
    }
}

impl SetUserEmojiStatus<'_> {
    pub fn custom_emoji_id(mut self, custom_emoji_id: i64) -> Self {
        self.custom_emoji_id = Some(custom_emoji_id);
        self
    }

    pub fn expiration_date(mut self, date: i32) -> Self {
        self.expiration_date = Some(date);
        self
    }

    async fn send(self) -> Result<bool> {
        let emoji_status = match self.custom_emoji_id {
            Some(document_id) => base::EmojiStatus::EmojiStatus(Box::new(types::EmojiStatus {
                document_id,
                until: self.expiration_date,
            })),
            None => base::EmojiStatus::EmojiStatusEmpty,
        };
        let user_id = self.client.resolve_user(&self.user_id).await?;
        self.client.invoke(&functions::bots::UpdateUserEmojiStatus { user_id, emoji_status }).await
    }
}

into_future!(SetUserEmojiStatus, bool);
