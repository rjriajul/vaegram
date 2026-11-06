use crate::types::{InputPollMedia, Text};

/// An answer option of a poll to send. A plain `&str`, `String` or [`Text`] converts into one without media.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct InputPollOption<'a> {
    /// The option's text. Telegram currently accepts only custom emoji entities here; a plain string is parsed with
    /// the client's default parse mode, so pass `Text::new().plain(..)` to send it as is.
    pub text: Text,
    /// Media shown with the option. `None` for a text-only option.
    pub media: Option<InputPollMedia<'a>>,
}

impl<'a> InputPollOption<'a> {
    /// Creates an option with the given text and no media.
    pub fn new(text: impl Into<Text>) -> Self {
        InputPollOption { text: text.into(), media: None }
    }

    /// Attaches media to the option.
    #[must_use]
    pub fn media(mut self, media: impl Into<InputPollMedia<'a>>) -> Self {
        self.media = Some(media.into());
        self
    }
}

impl From<&str> for InputPollOption<'_> {
    fn from(text: &str) -> Self {
        InputPollOption::new(text)
    }
}

impl From<String> for InputPollOption<'_> {
    fn from(text: String) -> Self {
        InputPollOption::new(text)
    }
}

impl From<Text> for InputPollOption<'_> {
    fn from(text: Text) -> Self {
        InputPollOption::new(text)
    }
}
