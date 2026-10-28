use crate::types::InputPollMedia;

pub struct InputPollOption<'a> {
    pub text: String,
    pub media: Option<InputPollMedia<'a>>,
}

impl<'a> InputPollOption<'a> {
    pub fn new(text: impl Into<String>) -> Self {
        InputPollOption { text: text.into(), media: None }
    }

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
