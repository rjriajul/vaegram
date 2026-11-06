use crate::Client;
use crate::errors::{Error, Result};
use crate::methods::accepted;
use crate::raw::functions;
use crate::types::{BotPreviewMedia, PeerRef};

/// Request built by [`Client::reorder_bot_preview_media`].
#[must_use = "a request does nothing until it is awaited"]
pub struct ReorderBotPreviewMedia<'a> {
    client: &'a Client,
    bot_id: PeerRef,
    order: &'a [BotPreviewMedia],
    language_code: String,
}

impl Client {
    /// Sets the order of a bot's default preview media, the ones shown in languages without previews of their own,
    /// to `order`.
    ///
    /// [`language_code`](ReorderBotPreviewMedia::language_code) reorders one language's previews instead. Fails with
    /// `Error::InvalidArgument` if `order` is empty or a preview has no usable media.
    pub fn reorder_bot_preview_media<'a>(
        &'a self,
        bot_id: impl Into<PeerRef>,
        order: &'a [BotPreviewMedia],
    ) -> ReorderBotPreviewMedia<'a> {
        ReorderBotPreviewMedia { client: self, bot_id: bot_id.into(), order, language_code: String::new() }
    }
}

impl ReorderBotPreviewMedia<'_> {
    /// Reorders the previews for users with this two-letter language code.
    pub fn language_code(mut self, language_code: impl Into<String>) -> Self {
        self.language_code = language_code.into();
        self
    }

    async fn send(self) -> Result<()> {
        if self.order.is_empty() {
            return Err(Error::InvalidArgument("reorder_bot_preview_media needs at least one preview"));
        }
        let order = self.order.iter().map(BotPreviewMedia::input).collect::<Result<_>>()?;
        let bot = self.client.resolve_user(&self.bot_id).await?;
        let request = functions::bots::ReorderPreviewMedias { bot, lang_code: self.language_code, order };
        accepted(self.client.invoke(&request).await?)
    }
}

into_future!(ReorderBotPreviewMedia, ());

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
        assert!(matches!(client.reorder_bot_preview_media(42, &[]).await, Err(Error::InvalidArgument(_))));
        let bare = [BotPreviewMedia { date: 1, media: None }];
        let reordered = client.reorder_bot_preview_media(42, &bare).language_code("en").await;
        assert!(matches!(reordered, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}
