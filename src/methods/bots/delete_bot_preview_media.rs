use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::{BotPreviewMedia, PeerRef};

impl Client {
    pub async fn delete_bot_preview_media(
        &self,
        bot_id: impl Into<PeerRef>,
        lang_code: &str,
        previews: &[BotPreviewMedia],
    ) -> Result<bool> {
        if previews.is_empty() {
            return Err(Error::InvalidArgument("delete_bot_preview_media takes at least one preview"));
        }
        let media = previews.iter().map(BotPreviewMedia::input).collect::<Result<_>>()?;
        let bot = self.resolve_user(&bot_id.into()).await?;
        self.invoke(&functions::bots::DeletePreviewMedia { bot, lang_code: lang_code.into(), media }).await
    }
}

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
        assert!(matches!(client.delete_bot_preview_media(42, "en", &[]).await, Err(Error::InvalidArgument(_))));
        let bare = BotPreviewMedia { date: 1, media: None };
        assert!(matches!(client.delete_bot_preview_media(42, "en", &[bare]).await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}
