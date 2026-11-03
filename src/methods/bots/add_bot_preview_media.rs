use crate::Client;
use crate::enums::MediaKind;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{BotPreviewMedia, InputMedia, PeerRef};

impl Client {
    pub async fn add_bot_preview_media(
        &self,
        bot_id: impl Into<PeerRef>,
        lang_code: &str,
        media: &InputMedia<'_>,
    ) -> Result<BotPreviewMedia> {
        let bot = self.resolve_user(&bot_id.into()).await?;
        let media = self.upload_preview(media).await?;
        let request = functions::bots::AddPreviewMedia { bot, lang_code: lang_code.into(), media };
        Ok(BotPreviewMedia::parse(self, self.invoke(&request).await?))
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
        assert!(matches!(client.add_bot_preview_media(42, "en", &document).await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}
