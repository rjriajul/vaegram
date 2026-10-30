use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;

impl Client {
    pub async fn reorder_folders(&self, folder_ids: &[i32]) -> Result<()> {
        if folder_ids.is_empty() {
            return Err(Error::InvalidArgument("reorder_folders needs at least one folder id"));
        }
        self.invoke(&functions::messages::UpdateDialogFiltersOrder { order: folder_ids.to_vec() }).await?;
        Ok(())
    }
}
