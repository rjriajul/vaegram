use super::inline_query_result::{Parts, WebFile, setters};

#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultPhoto {
    parts: Parts,
    file: WebFile,
}

impl InlineQueryResultPhoto {
    pub fn new(photo_url: impl Into<String>, thumbnail_url: impl Into<String>) -> Self {
        let parts = Parts { thumbnail_url: Some(thumbnail_url.into()), ..Parts::new() };
        InlineQueryResultPhoto { parts, file: WebFile::new(photo_url.into(), "image/jpeg") }
    }

    pub(super) fn parts(self) -> Parts {
        Parts { file: Some(self.file), ..self.parts }
    }
}

setters!(
    InlineQueryResultPhoto:
    id, reply_markup, input_message_content, caption, caption_above, title, description, dimensions
);
