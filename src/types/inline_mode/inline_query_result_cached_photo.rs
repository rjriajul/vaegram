use crate::types::Photo;

use super::inline_query_result::{Parts, Stored, setters};

#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultCachedPhoto {
    parts: Parts,
}

impl InlineQueryResultCachedPhoto {
    pub fn new(photo: &Photo) -> Self {
        InlineQueryResultCachedPhoto { parts: Parts { stored: Some(Stored::photo(photo)), ..Parts::new() } }
    }

    pub(super) fn parts(self) -> Parts {
        self.parts
    }
}

setters!(
    InlineQueryResultCachedPhoto:
    id, reply_markup, input_message_content, caption, caption_above, title, description
);
