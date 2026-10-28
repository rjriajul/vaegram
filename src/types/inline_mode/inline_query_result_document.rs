use super::inline_query_result::{Parts, WebFile, setters};

/// A ZIP file, or a PDF when `mime_type("application/pdf")` is set.
#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultDocument {
    parts: Parts,
    file: WebFile,
}

impl InlineQueryResultDocument {
    pub fn new(document_url: impl Into<String>, title: impl Into<String>) -> Self {
        let parts = Parts { title: Some(title.into()), ..Parts::new() };
        InlineQueryResultDocument { parts, file: WebFile::new(document_url.into(), "application/zip") }
    }

    pub(super) fn parts(self) -> Parts {
        Parts { file: Some(self.file), ..self.parts }
    }
}

setters!(
    InlineQueryResultDocument:
    id, reply_markup, input_message_content, caption, description, thumbnail, mime_type
);
