use crate::Client;
use crate::errors::Result;
use crate::methods::accepted;
use crate::raw::functions;
use crate::types::ChatBackground;

impl Client {
    /// Removes a wallpaper from the account's saved list.
    pub async fn unsave_wallpaper(&self, wallpaper: &ChatBackground) -> Result<()> {
        let (settings, wallpaper) = (wallpaper.write_settings(), wallpaper.write());
        accepted(self.invoke(&functions::account::SaveWallPaper { wallpaper, unsave: true, settings }).await?)
    }
}
