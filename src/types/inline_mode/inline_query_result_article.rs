use crate::crypto::random;
use crate::enums::ParseMode;
use crate::types::ReplyMarkup;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineQueryResultArticle {
    pub id: String,
    pub title: String,
    pub text: String,
    pub description: Option<String>,
    pub url: Option<String>,
    pub parse_mode: ParseMode,
    pub reply_markup: Option<ReplyMarkup>,
}

impl InlineQueryResultArticle {
    pub fn new(title: impl Into<String>, text: impl Into<String>) -> Self {
        InlineQueryResultArticle {
            id: format!("{:016x}", u64::from_le_bytes(random())),
            title: title.into(),
            text: text.into(),
            description: None,
            url: None,
            parse_mode: ParseMode::Disabled,
            reply_markup: None,
        }
    }

    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = id.into();
        self
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn url(mut self, url: impl Into<String>) -> Self {
        self.url = Some(url.into());
        self
    }

    pub fn html(mut self) -> Self {
        self.parse_mode = ParseMode::Html;
        self
    }

    pub fn markdown(mut self) -> Self {
        self.parse_mode = ParseMode::Markdown;
        self
    }

    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
        self
    }
}
