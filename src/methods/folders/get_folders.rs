use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Folder;

impl Client {
    pub async fn get_folders(&self) -> Result<Vec<Folder>> {
        let base::messages::DialogFilters::DialogFilters(f) =
            self.invoke(&functions::messages::GetDialogFilters {}).await?;
        Ok(f.filters.into_iter().filter_map(|filter| Folder::parse(self, filter)).collect())
    }
}
