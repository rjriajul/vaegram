use super::inline_query_result::{Parts, WebFile, setters};

#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultMpeg4Gif {
    parts: Parts,
    file: WebFile,
}

impl InlineQueryResultMpeg4Gif {
    pub fn new(mpeg4_url: impl Into<String>, thumbnail_url: impl Into<String>) -> Self {
        let parts = Parts { thumbnail_url: Some(thumbnail_url.into()), ..Parts::new() };
        InlineQueryResultMpeg4Gif { parts, file: WebFile::new(mpeg4_url.into(), "video/mp4") }
    }

    pub(super) fn parts(self) -> Parts {
        Parts { file: Some(self.file), ..self.parts }
    }
}

setters!(
    InlineQueryResultMpeg4Gif:
    id, reply_markup, input_message_content, caption, caption_above, title, dimensions, duration
);
