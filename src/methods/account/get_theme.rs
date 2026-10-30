use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{THEME_FORMAT, Theme};

impl Client {
    pub async fn get_theme(&self, slug: impl Into<String>) -> Result<Theme> {
        let theme = base::InputTheme::InputThemeSlug(Box::new(types::InputThemeSlug { slug: slug.into() }));
        Ok(Theme::parse(self.invoke(&functions::account::GetTheme { format: THEME_FORMAT.into(), theme }).await?))
    }
}
