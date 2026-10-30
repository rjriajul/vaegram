use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn clear_all_drafts(&self) -> Result<()> {
        self.invoke(&functions::messages::ClearAllDrafts {}).await?;
        Ok(())
    }
}
