use crate::Client;
use crate::crypto::random;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::{base, types};
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

    pub(crate) async fn write(self, client: &Client) -> Result<base::InputBotInlineResult> {
        let (message, entities) = client.parse_text(&self.text, Some(self.parse_mode)).await?;
        let send_message = types::InputBotInlineMessageText {
            no_webpage: false,
            invert_media: false,
            message,
            entities: (!entities.is_empty()).then_some(entities),
            reply_markup: self.reply_markup.as_ref().map(ReplyMarkup::write),
        };
        Ok(base::InputBotInlineResult::InputBotInlineResult(Box::new(types::InputBotInlineResult {
            id: self.id,
            r#type: "article".into(),
            title: Some(self.title),
            description: self.description,
            url: self.url,
            thumb: None,
            content: None,
            send_message: base::InputBotInlineMessage::InputBotInlineMessageText(Box::new(send_message)),
        })))
    }
}
