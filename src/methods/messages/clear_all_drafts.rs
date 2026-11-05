use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Deletes the account's drafts in every chat.
    pub async fn clear_all_drafts(&self) -> Result<()> {
        self.invoke(&functions::messages::ClearAllDrafts {}).await?;
        Ok(())
    }
}
