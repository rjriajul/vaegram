use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::ChatBackground;

impl Client {
    pub async fn install_wallpaper(&self, wallpaper: &ChatBackground) -> Result<bool> {
        let (settings, wallpaper) = (wallpaper.write_settings(), wallpaper.write());
        self.invoke(&functions::account::InstallWallPaper { wallpaper, settings }).await
    }
}
