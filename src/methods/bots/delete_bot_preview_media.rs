use crate::Client;
use crate::errors::{Error, Result};
use crate::methods::accepted;
use crate::raw::functions;
use crate::types::{BotPreviewMedia, PeerRef};

/// Request built by [`Client::delete_bot_preview_media`].
#[must_use = "a request does nothing until it is awaited"]
pub struct DeleteBotPreviewMedia<'a> {
    client: &'a Client,
    bot_id: PeerRef,
    previews: &'a [BotPreviewMedia],
    language_code: String,
}

impl Client {
    /// Deletes default preview media of a bot you own, the ones shown in languages without previews of their own.
    ///
    /// [`language_code`](DeleteBotPreviewMedia::language_code) deletes from one language's previews instead. Fails
    /// with `Error::InvalidArgument` if `previews` is empty or one of them holds no media.
    pub fn delete_bot_preview_media<'a>(
        &'a self,
        bot_id: impl Into<PeerRef>,
        previews: &'a [BotPreviewMedia],
    ) -> DeleteBotPreviewMedia<'a> {
        DeleteBotPreviewMedia { client: self, bot_id: bot_id.into(), previews, language_code: String::new() }
    }
}

impl DeleteBotPreviewMedia<'_> {
    /// Deletes from the previews for users with this two-letter language code.
    pub fn language_code(mut self, language_code: impl Into<String>) -> Self {
        self.language_code = language_code.into();
        self
    }

    async fn send(self) -> Result<()> {
        if self.previews.is_empty() {
            return Err(Error::InvalidArgument("delete_bot_preview_media needs at least one preview"));
        }
        let media = self.previews.iter().map(BotPreviewMedia::input).collect::<Result<_>>()?;
        let bot = self.client.resolve_user(&self.bot_id).await?;
        let request = functions::bots::DeletePreviewMedia { bot, lang_code: self.language_code, media };
        accepted(self.client.invoke(&request).await?)
    }
}

into_future!(DeleteBotPreviewMedia, ());

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{base, types};
    use crate::types::BotPreviewMedia;

    #[tokio::test]
    async fn no_previews_or_unusable_ones_are_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        let peer = types::InputPeerUser { user_id: 42, access_hash: 7 };
        client.0.peers.lock().unwrap().insert(42, base::InputPeer::InputPeerUser(Box::new(peer)));
        assert!(matches!(client.delete_bot_preview_media(42, &[]).await, Err(Error::InvalidArgument(_))));
        let bare = [BotPreviewMedia { date: 1, media: None }];
        let deleted = client.delete_bot_preview_media(42, &bare).language_code("en").await;
        assert!(matches!(deleted, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}
