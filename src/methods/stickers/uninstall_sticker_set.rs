use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::utils::input_sticker_set;

impl Client {
    pub async fn uninstall_sticker_set(&self, name: &str) -> Result<()> {
        self.invoke(&functions::messages::UninstallStickerSet { stickerset: input_sticker_set(name) }).await?;
        Ok(())
    }
}
