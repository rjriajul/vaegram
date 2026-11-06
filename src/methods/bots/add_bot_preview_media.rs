use crate::Client;
use crate::enums::MediaKind;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{BotPreviewMedia, InputMedia, PeerRef};

/// Request built by [`Client::add_bot_preview_media`].
#[must_use = "a request does nothing until it is awaited"]
pub struct AddBotPreviewMedia<'a> {
    client: &'a Client,
    bot_id: PeerRef,
    media: InputMedia<'a>,
    language_code: String,
}

impl Client {
    /// Adds a photo or video to the default preview media of your bot, shown in languages without previews of
    /// their own.
    ///
    /// [`language_code`](AddBotPreviewMedia::language_code) adds it to one language's previews instead. Returns
    /// the new [`BotPreviewMedia`]. Any other media kind fails with `Error::InvalidArgument`.
    pub fn add_bot_preview_media<'a>(
        &'a self,
        bot_id: impl Into<PeerRef>,
        media: InputMedia<'a>,
    ) -> AddBotPreviewMedia<'a> {
        AddBotPreviewMedia { client: self, bot_id: bot_id.into(), media, language_code: String::new() }
    }

    /// Previews are photos or videos, uploaded through `messages.uploadMedia` against the account's own chat.
    pub(super) async fn upload_preview(&self, media: &InputMedia<'_>) -> Result<base::InputMedia> {
        if !matches!(media.kind, MediaKind::Photo | MediaKind::Video) {
            return Err(Error::InvalidArgument("a bot preview is a photo or a video"));
        }
        let peer = base::InputPeer::InputPeerSelf;
        self.upload_media(&peer, self.input_media(&peer, media, None, None).await?, None).await
    }
}

impl AddBotPreviewMedia<'_> {
    /// Adds the media to the previews for users with this two-letter language code.
    pub fn language_code(mut self, language_code: impl Into<String>) -> Self {
        self.language_code = language_code.into();
        self
    }

    async fn send(self) -> Result<BotPreviewMedia> {
        let bot = self.client.resolve_user(&self.bot_id).await?;
        let media = self.client.upload_preview(&self.media).await?;
        let request = functions::bots::AddPreviewMedia { bot, lang_code: self.language_code, media };
        Ok(BotPreviewMedia::parse(self.client, self.client.invoke(&request).await?))
    }
}

into_future!(AddBotPreviewMedia, BotPreviewMedia);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{base, types};
    use crate::types::InputMedia;

    #[tokio::test]
    async fn only_photos_and_videos_are_previews() {
        let (client, mut rx) = Client::recording();
        let peer = types::InputPeerUser { user_id: 42, access_hash: 7 };
        client.0.peers.lock().unwrap().insert(42, base::InputPeer::InputPeerUser(Box::new(peer)));
        let document = InputMedia::document("notes.txt");
        let added = client.add_bot_preview_media(42, document).language_code("en").await;
        assert!(matches!(added, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}
