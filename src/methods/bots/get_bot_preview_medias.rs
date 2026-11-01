use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{BotPreviewMedia, PeerRef};

impl Client {
    pub async fn get_bot_preview_medias(&self, bot_id: impl Into<PeerRef>) -> Result<Vec<BotPreviewMedia>> {
        let bot = self.resolve_user(&bot_id.into()).await?;
        let medias = self.invoke(&functions::bots::GetPreviewMedias { bot }).await?;
        Ok(medias.into_iter().map(|m| BotPreviewMedia::parse(self, m)).collect())
    }
}
