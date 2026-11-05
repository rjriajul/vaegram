use super::inline_query_result::{Parts, WebFile, setters};

/// A GIF file at a URL.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultGif {
    parts: Parts,
    file: WebFile,
}

impl InlineQueryResultGif {
    /// Creates a result for the GIF at `gif_url`, previewed by the image at `thumbnail_url`.
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
