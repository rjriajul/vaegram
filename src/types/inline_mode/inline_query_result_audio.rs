use super::inline_query_result::{Parts, WebFile, setters};

#[derive(Debug, Clone, PartialEq)]
pub struct InlineQueryResultAudio {
    parts: Parts,
    file: WebFile,
}

impl InlineQueryResultAudio {
    pub fn new(audio_url: impl Into<String>, title: impl Into<String>) -> Self {
        let parts = Parts { title: Some(title.into()), ..Parts::new() };
        InlineQueryResultAudio { parts, file: WebFile::new(audio_url.into(), "audio/mpeg") }
    }

    pub(super) fn parts(self) -> Parts {
        Parts { file: Some(self.file), ..self.parts }
    }
}

setters!(InlineQueryResultAudio: id, reply_markup, input_message_content, caption, performer, duration);
