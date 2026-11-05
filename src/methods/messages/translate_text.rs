use super::send_options::caption_setters;
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::{Text, TranslatedText};

/// Request built by [`Client::translate_text`]. Await it to send.
#[must_use = "a request does nothing until it is awaited"]
pub struct TranslateText<'a> {
    client: &'a Client,
    to_language_code: String,
    text: Text,
    parse_mode: Option<ParseMode>,
}

impl Client {
    /// Translates a piece of text into `to_language_code`.
    ///
    /// Awaiting the request returns the [`TranslatedText`].
    pub fn translate_text(&self, to_language_code: impl Into<String>, text: impl Into<Text>) -> TranslateText<'_> {
        TranslateText { client: self, to_language_code: to_language_code.into(), text: text.into(), parse_mode: None }
    }
}

impl TranslateText<'_> {
    async fn send(self) -> Result<TranslatedText> {
        let (text, entities) = self.client.write_text(self.text, self.parse_mode).await?;
        let text = types::TextWithEntities { text, entities };
        let request = functions::messages::TranslateText {
            peer: None,
            id: None,
            text: Some(vec![base::TextWithEntities::TextWithEntities(Box::new(text))]),
            to_lang: self.to_language_code,
            tone: None,
        };
        let result = self.client.invoke(&request).await?;
        TranslatedText::parse_all(result).pop().ok_or(Error::Unexpected("Telegram sent no translation back"))
    }
}

caption_setters!(TranslateText);
into_future!(TranslateText, TranslatedText);
