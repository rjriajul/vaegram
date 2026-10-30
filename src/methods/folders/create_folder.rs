use crate::Client;
use crate::errors::Result;
use crate::types::Folder;

// Ids 0 and 1 are taken by "All chats" and the archive. Gaps left by deleted folders get reused.
const FIRST_ID: i32 = 2;

impl Client {
    pub async fn create_folder(&self, mut folder: Folder) -> Result<Folder> {
        let taken = self.get_folders().await?;
        folder.id = FIRST_ID;
        while taken.iter().any(|f| f.id == folder.id) {
            folder.id += 1;
        }
        self.edit_folder(&folder).await?;
        Ok(folder)
    }
}
