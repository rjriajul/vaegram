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

    /// An article whose wire `title` stays unset, so Telegram lists it by the message text.
    pub(crate) fn untitled(content: impl Into<InputMessageContent>) -> Self {
        InlineQueryResultArticle { parts: Parts { content: Some(content.into()), ..Parts::new() } }
    }

    pub(super) fn parts(self) -> Parts {
        self.parts
    }
}

setters!(InlineQueryResultArticle: id, reply_markup, url, description, thumbnail);
