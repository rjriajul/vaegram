use super::inline_query_result::{Parts, WebFile, setters};

#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultVoice {
    parts: Parts,
    file: WebFile,
}

impl InlineQueryResultVoice {
    pub fn new(voice_url: impl Into<String>, title: impl Into<String>) -> Self {
        let parts = Parts { title: Some(title.into()), ..Parts::new() };
        InlineQueryResultVoice { parts, file: WebFile::new(voice_url.into(), "audio/ogg") }
    }

    pub(super) fn parts(self) -> Parts {
        Parts { file: Some(self.file), ..self.parts }
    }
}

setters!(InlineQueryResultVoice: id, reply_markup, input_message_content, caption, duration);
