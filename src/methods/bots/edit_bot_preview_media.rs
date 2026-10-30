use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{BotPreviewMedia, InputMedia, PeerRef};

impl Client {
    pub async fn edit_bot_preview_media(
        &self,
        bot_id: impl Into<PeerRef>,
        lang_code: &str,
        old: &BotPreviewMedia,
        new: &InputMedia<'_>,
    ) -> Result<BotPreviewMedia> {
        let media = old.input()?;
        let bot = self.resolve_user(&bot_id.into()).await?;
        let new_media = self.upload_preview(new).await?;
        let request = functions::bots::EditPreviewMedia { bot, lang_code: lang_code.into(), media, new_media };
        Ok(BotPreviewMedia::parse(self.invoke(&request).await?))
    }
}
