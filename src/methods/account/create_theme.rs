use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{Theme, ThemeSettings};

#[must_use = "a request does nothing until it is awaited"]
pub struct CreateTheme<'a> {
    client: &'a Client,
    title: String,
    settings: Vec<base::InputThemeSettings>,
    slug: String,
}

impl Client {
    pub fn create_theme(
        &self,
        title: impl Into<String>,
        settings: impl IntoIterator<Item = ThemeSettings>,
    ) -> CreateTheme<'_> {
        let settings = settings.into_iter().map(|s| s.write()).collect();
        CreateTheme { client: self, title: title.into(), settings, slug: String::new() }
    }
}

impl CreateTheme<'_> {
    pub fn slug(mut self, slug: impl Into<String>) -> Self {
        self.slug = slug.into();
        self
    }

    async fn send(self) -> Result<Theme> {
        // No theme file is ever sent, so the settings are the whole theme.
        if self.settings.is_empty() {
            return Err(Error::InvalidArgument("a theme needs at least one ThemeSettings"));
        }
        let request = functions::account::CreateTheme {
            slug: self.slug,
            title: self.title,
            document: None,
            settings: Some(self.settings),
        };
        Ok(Theme::parse(self.client.invoke(&request).await?))
    }
}

into_future!(CreateTheme, Theme);

#[cfg(test)]
mod tests {
    use crate::enums::BaseTheme;
    use crate::raw::{Identifiable, functions::account};
    use crate::types::ThemeSettings;
    use crate::{Client, Error};

    #[tokio::test]
    async fn a_theme_without_settings_is_refused_without_a_request() {
        let (client, mut rx) = Client::recording();
        let refused = client.create_theme("Empty", Vec::<ThemeSettings>::new()).await;
        assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{refused:?}");
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn settings_are_sent_with_the_slug() {
        let (client, mut rx) = Client::recording();
        let _ = client.create_theme("Night", [ThemeSettings::new(BaseTheme::Night, 1)]).slug("night").await;
        let body = rx.try_recv().expect("no request sent");
        let word = |at: usize| u32::from_le_bytes(body[at..at + 4].try_into().unwrap());
        // flags.3 is the settings vector; the document bit stays clear.
        assert_eq!((word(0), word(4)), (account::CreateTheme::CONSTRUCTOR_ID, 0b1000));
    }
}
