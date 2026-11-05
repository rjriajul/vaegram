use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::ChatBackground;

impl Client {
    /// Applies a wallpaper without saving it to the account's list.
    pub async fn install_wallpaper(&self, wallpaper: &ChatBackground) -> Result<bool> {
        let (settings, wallpaper) = (wallpaper.write_settings(), wallpaper.write());
        self.invoke(&functions::account::InstallWallPaper { wallpaper, settings }).await
    }
}
