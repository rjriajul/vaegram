use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::base;
use crate::types::InputRichMessage;

#[derive(Debug, Clone, PartialEq)]
pub enum MessageText {
    Text(String),
    Rich(InputRichMessage),
}

impl MessageText {
    pub(crate) async fn write(
        self,
        client: &Client,
        parse_mode: Option<ParseMode>,
    ) -> Result<(Option<String>, Option<Vec<base::MessageEntity>>, Option<base::InputRichMessage>)> {
        Ok(match self {
            MessageText::Text(text) => {
                let (message, entities) = client.parse_text(&text, parse_mode).await?;
                (Some(message), (!entities.is_empty()).then_some(entities), None)
            }
            MessageText::Rich(rich) => (None, None, Some(rich.write())),
        })
    }
}

impl From<&str> for MessageText {
    fn from(text: &str) -> Self {
        MessageText::Text(text.into())
    }
}

impl From<String> for MessageText {
    fn from(text: String) -> Self {
        MessageText::Text(text)
    }
}

impl From<InputRichMessage> for MessageText {
    fn from(rich: InputRichMessage) -> Self {
        MessageText::Rich(rich)
    }
}
