use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::utils::input_sticker_set;

impl Client {
    /// Uninstalls a sticker set, found by its `name`, from the account.
    pub async fn uninstall_sticker_set(&self, name: &str) -> Result<()> {
        self.invoke(&functions::messages::UninstallStickerSet { stickerset: input_sticker_set(name) }).await?;
        Ok(())
    }
}
