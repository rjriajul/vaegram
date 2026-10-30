use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::utils::input_sticker_set;

impl Client {
    pub async fn archive_sticker_set(&self, name: &str) -> Result<()> {
        self.toggle_sticker_set_archived(name, true).await
    }

    pub(crate) async fn toggle_sticker_set_archived(&self, name: &str, archive: bool) -> Result<()> {
        let stickersets = vec![input_sticker_set(name)];
        let unarchive = !archive;
        let request = functions::messages::ToggleStickerSets { uninstall: false, archive, unarchive, stickersets };
        self.invoke(&request).await?;
        Ok(())
    }
}
