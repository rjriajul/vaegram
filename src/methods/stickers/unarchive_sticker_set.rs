use crate::Client;
use crate::errors::Result;

impl Client {
    /// Moves an archived sticker set, found by its `name`, back to the installed sets.
    pub async fn unarchive_sticker_set(&self, name: &str) -> Result<()> {
        self.toggle_sticker_set_archived(name, false).await
    }
}
