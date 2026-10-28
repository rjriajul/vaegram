use crate::types::Animation;

use super::inline_query_result::{Parts, Stored, setters};

#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultCachedMpeg4Gif {
    parts: Parts,
}

impl InlineQueryResultCachedMpeg4Gif {
    pub fn new(animation: &Animation) -> Self {
        let parts = Parts { stored: Some(Stored::document(&animation.location)), ..Parts::new() };
        InlineQueryResultCachedMpeg4Gif { parts }
    }

    pub(super) fn parts(self) -> Parts {
        self.parts
    }
}

setters!(InlineQueryResultCachedMpeg4Gif: id, reply_markup, input_message_content, caption, caption_above, title);
