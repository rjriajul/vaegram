use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    pub async fn reset_wallpapers(&self) -> Result<bool> {
        self.invoke(&functions::account::ResetWallPapers {}).await
    }
}
