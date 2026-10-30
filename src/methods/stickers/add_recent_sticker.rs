use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::Sticker;

impl Client {
    pub async fn add_recent_sticker(&self, sticker: &Sticker) -> Result<()> {
        let id = sticker.input_document();
        let request = functions::messages::SaveRecentSticker { attached: false, id, unsave: false };
        self.invoke(&request).await?;
        Ok(())
    }
}
