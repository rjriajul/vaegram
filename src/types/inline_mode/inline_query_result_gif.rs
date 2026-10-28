use super::inline_query_result::{Parts, WebFile, setters};

#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultGif {
    parts: Parts,
    file: WebFile,
}

impl InlineQueryResultGif {
    pub fn new(gif_url: impl Into<String>, thumbnail_url: impl Into<String>) -> Self {
        let parts = Parts { thumbnail_url: Some(thumbnail_url.into()), ..Parts::new() };
        InlineQueryResultGif { parts, file: WebFile::new(gif_url.into(), "image/gif") }
    }

    pub(super) fn parts(self) -> Parts {
        Parts { file: Some(self.file), ..self.parts }
    }
}

setters!(
    InlineQueryResultGif:
    id, reply_markup, input_message_content, caption, caption_above, title, dimensions, duration
);
