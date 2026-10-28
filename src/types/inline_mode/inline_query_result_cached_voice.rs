use crate::types::Voice;

use super::inline_query_result::{Parts, Stored, setters};

#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultCachedVoice {
    parts: Parts,
}

impl InlineQueryResultCachedVoice {
    pub fn new(voice: &Voice, title: impl Into<String>) -> Self {
        let parts =
            Parts { title: Some(title.into()), stored: Some(Stored::document(&voice.location)), ..Parts::new() };
        InlineQueryResultCachedVoice { parts }
    }

    pub(super) fn parts(self) -> Parts {
        self.parts
    }
}

setters!(InlineQueryResultCachedVoice: id, reply_markup, input_message_content, caption);
