use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{BotPreviewMedia, PeerRef};

impl Client {
    /// Returns a bot's preview media for the current account's language.
    pub async fn get_bot_preview_media(&self, bot_id: impl Into<PeerRef>) -> Result<Vec<BotPreviewMedia>> {
        let bot = self.resolve_user(&bot_id.into()).await?;
        let media = self.invoke(&functions::bots::GetPreviewMedias { bot }).await?;
        Ok(media.into_iter().map(|m| BotPreviewMedia::parse(self, m)).collect())
    }
}
