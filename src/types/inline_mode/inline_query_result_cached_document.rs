use crate::types::Document;

use super::inline_query_result::{Parts, Stored, setters};

#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultCachedDocument {
    parts: Parts,
}

impl InlineQueryResultCachedDocument {
    pub fn new(document: &Document, title: impl Into<String>) -> Self {
        let parts =
            Parts { title: Some(title.into()), stored: Some(Stored::document(&document.location)), ..Parts::new() };
        InlineQueryResultCachedDocument { parts }
    }

    pub(super) fn parts(self) -> Parts {
        self.parts
    }
}

setters!(InlineQueryResultCachedDocument: id, reply_markup, input_message_content, caption, description);
