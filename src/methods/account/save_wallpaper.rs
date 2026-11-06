use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;
use crate::types::ChatBackground;

impl Client {
    /// Saves a wallpaper to the account's list.
    pub async fn save_wallpaper(&self, wallpaper: &ChatBackground) -> Result<()> {
        let (settings, wallpaper) = (wallpaper.write_settings(), wallpaper.write());
        accepted(self.invoke(&functions::account::SaveWallPaper { wallpaper, unsave: false, settings }).await?)
    }
}
