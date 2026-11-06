use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

/// Request built by [`Client::set_user_emoji_status`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SetUserEmojiStatus<'a> {
    client: &'a Client,
    user_id: PeerRef,
    custom_emoji_id: Option<i64>,
    until: Option<i32>,
}

impl Client {
    /// Sets the emoji status of a user who allowed the bot to change it.
    ///
    /// Without [`custom_emoji_id`](SetUserEmojiStatus::custom_emoji_id) it clears the status.
    pub fn set_user_emoji_status(&self, user_id: impl Into<PeerRef>) -> SetUserEmojiStatus<'_> {
        SetUserEmojiStatus { client: self, user_id: user_id.into(), custom_emoji_id: None, until: None }
    }
}

impl SetUserEmojiStatus<'_> {
    /// Uses this custom emoji as the status.
    pub fn custom_emoji_id(mut self, custom_emoji_id: i64) -> Self {
        self.custom_emoji_id = Some(custom_emoji_id);
        self
    }

    /// Makes the status expire at this Unix time, as [`EmojiStatus::until`] reads it back. Ignored without a custom
    /// emoji.
    ///
    /// [`EmojiStatus::until`]: crate::types::EmojiStatus::until
    pub fn until(mut self, date: i32) -> Self {
        self.until = Some(date);
        self
    }

    async fn send(self) -> Result<()> {
        let emoji_status = match self.custom_emoji_id {
            Some(document_id) => {
                base::EmojiStatus::EmojiStatus(Box::new(types::EmojiStatus { document_id, until: self.until }))
            }
            None => base::EmojiStatus::EmojiStatusEmpty,
        };
        let user_id = self.client.resolve_user(&self.user_id).await?;
        accepted(self.client.invoke(&functions::bots::UpdateUserEmojiStatus { user_id, emoji_status }).await?)
    }
}

into_future!(SetUserEmojiStatus, ());

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn an_expiring_status_is_sent_with_its_end() {
        let (client, mut rx) = Client::recording();
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        let _ = client.set_user_emoji_status(7).custom_emoji_id(5).until(9).await;
        let user_id = base::InputUser::InputUser(Box::new(types::InputUser { user_id: 7, access_hash: 1 }));
        let status = types::EmojiStatus { document_id: 5, until: Some(9) };
        let emoji_status = base::EmojiStatus::EmojiStatus(Box::new(status));
        assert_eq!(rx.try_recv().unwrap(), functions::bots::UpdateUserEmojiStatus { user_id, emoji_status }.to_bytes());
    }
}
