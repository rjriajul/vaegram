use crate::types::InputMessageContent;

use super::inline_query_result::{Parts, setters};

#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultContact {
    parts: Parts,
    phone_number: String,
    first_name: String,
    last_name: String,
    vcard: String,
}

impl InlineQueryResultContact {
    pub fn new(phone_number: impl Into<String>, first_name: impl Into<String>) -> Self {
        InlineQueryResultContact {
            parts: Parts::new(),
            phone_number: phone_number.into(),
            first_name: first_name.into(),
            last_name: String::new(),
            vcard: String::new(),
        }
    }

    pub fn last_name(mut self, last_name: impl Into<String>) -> Self {
        self.last_name = last_name.into();
        self
    }

    pub fn vcard(mut self, vcard: impl Into<String>) -> Self {
        self.vcard = vcard.into();
        self
    }

    pub(super) fn parts(self) -> Parts {
        let title = if self.last_name.is_empty() {
            self.first_name.clone()
        } else {
            format!("{} {}", self.first_name, self.last_name)
        };
        let contact = InputMessageContent::Contact {
            phone_number: self.phone_number,
            first_name: self.first_name,
            last_name: self.last_name,
            vcard: self.vcard,
        };
        Parts { title: Some(title), content: Some(self.parts.content.unwrap_or(contact)), ..self.parts }
    }
}

setters!(InlineQueryResultContact: id, reply_markup, input_message_content, thumbnail);
