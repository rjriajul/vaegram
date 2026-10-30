use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::ChatBackground;

impl Client {
    pub async fn get_wallpaper(&self, slug: impl Into<String>) -> Result<ChatBackground> {
        let slug = types::InputWallPaperSlug { slug: slug.into() };
        let wallpaper = base::InputWallPaper::InputWallPaperSlug(Box::new(slug));
        Ok(ChatBackground::parse(self.invoke(&functions::account::GetWallPaper { wallpaper }).await?))
    }
}
