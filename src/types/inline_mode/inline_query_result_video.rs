use super::inline_query_result::{Parts, WebFile, setters};

/// An MP4 video, or a page with an embedded player when `mime_type("text/html")` is set.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultVideo {
    parts: Parts,
    file: WebFile,
}

impl InlineQueryResultVideo {
    pub fn new(video_url: impl Into<String>, thumbnail_url: impl Into<String>, title: impl Into<String>) -> Self {
        let parts = Parts { thumbnail_url: Some(thumbnail_url.into()), title: Some(title.into()), ..Parts::new() };
        InlineQueryResultVideo { parts, file: WebFile::new(video_url.into(), "video/mp4") }
    }

    pub(super) fn parts(self) -> Parts {
        Parts { file: Some(self.file), ..self.parts }
    }
}

setters!(
    InlineQueryResultVideo:
    id, reply_markup, input_message_content, caption, caption_above, description, dimensions, duration, mime_type
);
