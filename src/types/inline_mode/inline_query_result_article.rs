use crate::types::InputMessageContent;

use super::inline_query_result::{Parts, setters};

/// A link to an article or any other text content, shown as a title in the results list.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultArticle {
    parts: Parts,
}

impl InlineQueryResultArticle {
    /// Creates an article that sends `content` when picked. A `&str` or `String` becomes plain text.
    pub fn new(title: impl Into<String>, content: impl Into<InputMessageContent>) -> Self {
        let parts = Parts { title: Some(title.into()), content: Some(content.into()), ..Parts::new() };
        InlineQueryResultArticle { parts }
    }

    pub(super) fn parts(self) -> Parts {
        self.parts
    }
}

setters!(InlineQueryResultArticle: id, reply_markup, url, description, thumbnail);
