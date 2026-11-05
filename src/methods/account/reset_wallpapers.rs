use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Deletes all installed wallpapers and goes back to the default set.
    pub async fn reset_wallpapers(&self) -> Result<bool> {
        self.invoke(&functions::account::ResetWallPapers {}).await
    }
}
