use crate::types::Video;

use super::inline_query_result::{Parts, Stored, setters};

/// A [`Video`](crate::types::Video) already on Telegram's servers, offered as an inline result.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultCachedVideo {
    parts: Parts,
}

impl InlineQueryResultCachedVideo {
    /// Creates a result from `video`.
    pub fn new(video: &Video, title: impl Into<String>) -> Self {
        let parts =
            Parts { title: Some(title.into()), stored: Some(Stored::document(&video.location)), ..Parts::new() };
        InlineQueryResultCachedVideo { parts }
    }

    pub(super) fn parts(self) -> Parts {
        self.parts
    }
}

setters!(InlineQueryResultCachedVideo: id, reply_markup, input_message_content, caption, caption_above, description);
