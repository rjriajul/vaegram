use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn toggle_folder_tags(&self, enabled: bool) -> Result<()> {
        self.invoke(&functions::messages::ToggleDialogFilterTags { enabled }).await?;
        Ok(())
    }
}
