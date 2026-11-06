use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::ChatBackground;

impl Client {
    /// Removes a wallpaper from the account's saved list.
    pub async fn unsave_wallpaper(&self, wallpaper: &ChatBackground) -> Result<()> {
        let (settings, wallpaper) = (wallpaper.write_settings(), wallpaper.write());
        self.invoke(&functions::account::SaveWallPaper { wallpaper, unsave: true, settings }).await?;
        Ok(())
    }
}
