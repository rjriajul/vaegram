use crate::types::Animation;

use super::inline_query_result::{Parts, Stored, setters};

#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultCachedGif {
    parts: Parts,
}

impl InlineQueryResultCachedGif {
    pub fn new(animation: &Animation) -> Self {
        let parts = Parts { stored: Some(Stored::document(&animation.location)), ..Parts::new() };
        InlineQueryResultCachedGif { parts }
    }

    pub(super) fn parts(self) -> Parts {
        self.parts
    }
}

setters!(InlineQueryResultCachedGif: id, reply_markup, input_message_content, caption, caption_above, title);
