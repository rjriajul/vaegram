use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::Folder;

impl Client {
    pub async fn edit_folder(&self, folder: &Folder) -> Result<()> {
        let filter = Some(folder.write(self).await?);
        self.invoke(&functions::messages::UpdateDialogFilter { id: folder.id, filter }).await?;
        Ok(())
    }
}
