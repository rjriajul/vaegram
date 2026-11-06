use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

/// Request built by [`Client::set_user_emoji_status`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SetUserEmojiStatus<'a> {
    client: &'a Client,
    user_id: PeerRef,
    custom_emoji_id: Option<i64>,
    expiration_date: Option<i32>,
}

impl Client {
    /// Sets the emoji status of a user who allowed the bot to change it.
    ///
    /// Without [`custom_emoji_id`](SetUserEmojiStatus::custom_emoji_id) it clears the status.
    pub fn set_user_emoji_status(&self, user_id: impl Into<PeerRef>) -> SetUserEmojiStatus<'_> {
        SetUserEmojiStatus { client: self, user_id: user_id.into(), custom_emoji_id: None, expiration_date: None }
    }
}

impl SetUserEmojiStatus<'_> {
    /// Uses this custom emoji as the status.
    pub fn custom_emoji_id(mut self, custom_emoji_id: i64) -> Self {
        self.custom_emoji_id = Some(custom_emoji_id);
        self
    }

    /// Makes the status expire at this Unix time. Ignored without a custom emoji.
    pub fn expiration_date(mut self, date: i32) -> Self {
        self.expiration_date = Some(date);
        self
    }

    async fn send(self) -> Result<()> {
        let emoji_status = match self.custom_emoji_id {
            Some(document_id) => base::EmojiStatus::EmojiStatus(Box::new(types::EmojiStatus {
                document_id,
                until: self.expiration_date,
            })),
            None => base::EmojiStatus::EmojiStatusEmpty,
        };
        let user_id = self.client.resolve_user(&self.user_id).await?;
        self.client.invoke(&functions::bots::UpdateUserEmojiStatus { user_id, emoji_status }).await?;
        Ok(())
    }
}

into_future!(SetUserEmojiStatus, ());
