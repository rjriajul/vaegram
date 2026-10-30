use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::ChatBackground;

impl Client {
    pub async fn get_wallpapers(&self) -> Result<Vec<ChatBackground>> {
        match self.invoke(&functions::account::GetWallPapers { hash: 0 }).await? {
            base::account::WallPapers::WallPapers(w) => {
                Ok(w.wallpapers.into_iter().map(ChatBackground::parse).collect())
            }
            base::account::WallPapers::WallPapersNotModified => Ok(Vec::new()),
        }
    }
}
