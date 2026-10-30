use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn delete_folder(&self, folder_id: i32) -> Result<()> {
        self.invoke(&functions::messages::UpdateDialogFilter { id: folder_id, filter: None }).await?;
        Ok(())
    }
}
