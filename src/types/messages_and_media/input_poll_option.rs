use crate::types::InputPollMedia;

/// An answer option of a poll to send. A plain `&str` or `String` converts into one without media.
#[non_exhaustive]
pub struct InputPollOption<'a> {
    /// The option's text.
    pub text: String,
    /// Media shown with the option. `None` for a text-only option.
    pub media: Option<InputPollMedia<'a>>,
}

impl<'a> InputPollOption<'a> {
    /// Creates an option with the given text and no media.
    pub fn new(text: impl Into<String>) -> Self {
        InputPollOption { text: text.into(), media: None }
    }

    /// Attaches media to the option.
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
