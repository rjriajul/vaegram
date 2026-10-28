use super::block::for_each_text;
use super::rich_text::push_plain;
use crate::enums::ParseMode;
use crate::parser;
use crate::raw::{base, types};
use crate::types::{Document, Photo};

#[derive(Debug, Clone, PartialEq)]
pub struct RichMessage {
    pub rtl: bool,
    /// Telegram sent only part of the message; `messages.getRichMessage` returns all of it.
    pub part: bool,
    pub raw: Box<types::RichMessage>,
}

impl RichMessage {
    pub(crate) fn parse(raw: base::RichMessage) -> RichMessage {
        let base::RichMessage::RichMessage(raw) = raw;
        RichMessage { rtl: raw.rtl, part: raw.part, raw }
    }

    pub(crate) fn sent(raw: base::InputRichMessage) -> (String, Option<RichMessage>) {
        match raw {
            base::InputRichMessage::InputRichMessage(m) => {
                let m = *m;
                let raw =
                    types::RichMessage { rtl: m.rtl, part: false, blocks: m.blocks, photos: vec![], documents: vec![] };
                let rich = RichMessage::parse(base::RichMessage::RichMessage(Box::new(raw)));
                (rich.text(), Some(rich))
            }
            base::InputRichMessage::InputRichMessageHTML(m) => (parser::parse(&m.html, ParseMode::Html).0, None),
            base::InputRichMessage::InputRichMessageMarkdown(m) => {
                (parser::parse(&m.markdown, ParseMode::Markdown).0, None)
            }
        }
    }

    pub fn photo(&self, id: i64) -> Option<Photo> {
        let photo = self.raw.photos.iter().find(|p| match p {
            base::Photo::Photo(p) => p.id == id,
            base::Photo::PhotoEmpty(p) => p.id == id,
        });
        photo.and_then(Photo::parse)
    }

    pub fn document(&self, id: i64) -> Option<Document> {
        let document = self.raw.documents.iter().find(|d| match d {
            base::Document::Document(d) => d.id == id,
            base::Document::DocumentEmpty(d) => d.id == id,
        });
        document.and_then(Document::parse)
    }

    pub fn text(&self) -> String {
        let mut out = String::new();
        for block in &self.raw.blocks {
            for_each_text(block, &mut |text| {
                let start = out.len();
                if start > 0 {
                    out.push('\n');
                }
                push_plain(text, &mut out);
                if out.len() == start + usize::from(start > 0) {
                    out.truncate(start);
                }
            });
        }
        out
    }
}
