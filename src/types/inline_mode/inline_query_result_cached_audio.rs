use crate::types::Audio;

use super::inline_query_result::{Parts, Stored, setters};

/// An [`Audio`] already on Telegram's servers, offered as an inline result.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultCachedAudio {
    parts: Parts,
}

impl InlineQueryResultCachedAudio {
    /// Creates a result from `audio`.
    pub fn new(audio: &Audio) -> Self {
        let parts = Parts { stored: Some(Stored::document(&audio.location)), ..Parts::new() };
        InlineQueryResultCachedAudio { parts }
    }

    pub(super) fn parts(self) -> Parts {
        self.parts
    }
}

setters!(InlineQueryResultCachedAudio: id, reply_markup, input_message_content, caption);
