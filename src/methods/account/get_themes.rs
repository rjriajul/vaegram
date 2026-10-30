use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{THEME_FORMAT, Theme};

impl Client {
    pub async fn get_themes(&self) -> Result<Vec<Theme>> {
        match self.invoke(&functions::account::GetThemes { format: THEME_FORMAT.into(), hash: 0 }).await? {
            base::account::Themes::Themes(t) => Ok(t.themes.into_iter().map(Theme::parse).collect()),
            base::account::Themes::ThemesNotModified => Ok(Vec::new()),
        }
    }
}
