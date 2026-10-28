use crate::types::Sticker;

use super::inline_query_result::{Parts, Stored, setters};

#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultCachedSticker {
    parts: Parts,
}

impl InlineQueryResultCachedSticker {
    pub fn new(sticker: &Sticker) -> Self {
        let parts = Parts { stored: Some(Stored::document(&sticker.location)), ..Parts::new() };
        InlineQueryResultCachedSticker { parts }
    }

    pub(super) fn parts(self) -> Parts {
        self.parts
    }
}

setters!(InlineQueryResultCachedSticker: id, reply_markup, input_message_content);
