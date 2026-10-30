use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Theme;

impl Client {
    pub async fn get_chat_themes(&self) -> Result<Vec<Theme>> {
        match self.invoke(&functions::account::GetChatThemes { hash: 0 }).await? {
            base::account::Themes::Themes(t) => Ok(t.themes.into_iter().map(Theme::parse).collect()),
            base::account::Themes::ThemesNotModified => Ok(Vec::new()),
        }
    }
}
