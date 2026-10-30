use crate::Client;
use crate::enums::BaseTheme;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{THEME_FORMAT, Theme};

#[must_use = "a request does nothing until it is awaited"]
pub struct InstallTheme<'a> {
    client: &'a Client,
    theme: base::InputTheme,
    dark: bool,
    base_theme: Option<BaseTheme>,
}

impl Client {
    pub fn install_theme(&self, theme: &Theme) -> InstallTheme<'_> {
        InstallTheme { client: self, theme: theme.write(), dark: false, base_theme: None }
    }
}

impl InstallTheme<'_> {
    pub fn dark(mut self) -> Self {
        self.dark = true;
        self
    }

    pub fn base_theme(mut self, base_theme: BaseTheme) -> Self {
        self.base_theme = Some(base_theme);
        self
    }

    async fn send(self) -> Result<bool> {
        let request = functions::account::InstallTheme {
            dark: self.dark,
            theme: Some(self.theme),
            format: Some(THEME_FORMAT.into()),
            base_theme: self.base_theme.map(BaseTheme::write),
        };
        self.client.invoke(&request).await
    }
}

into_future!(InstallTheme, bool);
