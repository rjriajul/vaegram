use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::ChatBackground;

impl Client {
    pub async fn save_wallpaper(&self, wallpaper: &ChatBackground) -> Result<bool> {
        let (settings, wallpaper) = (wallpaper.write_settings(), wallpaper.write());
        self.invoke(&functions::account::SaveWallPaper { wallpaper, unsave: false, settings }).await
    }
}
