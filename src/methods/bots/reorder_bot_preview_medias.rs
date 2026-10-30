use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::{BotPreviewMedia, PeerRef};

impl Client {
    pub async fn reorder_bot_preview_medias(
        &self,
        bot_id: impl Into<PeerRef>,
        lang_code: &str,
        order: &[BotPreviewMedia],
    ) -> Result<bool> {
        if order.is_empty() {
            return Err(Error::InvalidArgument("reorder_bot_preview_medias takes at least one preview"));
        }
        let order = order.iter().map(BotPreviewMedia::input).collect::<Result<_>>()?;
        let bot = self.resolve_user(&bot_id.into()).await?;
        self.invoke(&functions::bots::ReorderPreviewMedias { bot, lang_code: lang_code.into(), order }).await
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
        assert!(matches!(client.reorder_bot_preview_medias(42, "en", &[]).await, Err(Error::InvalidArgument(_))));
        let bare = BotPreviewMedia { date: 1, media: None };
        assert!(matches!(client.reorder_bot_preview_medias(42, "en", &[bare]).await, Err(Error::InvalidArgument(_))));
        assert!(rx.try_recv().is_err());
    }
}
