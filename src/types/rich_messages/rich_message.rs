use std::sync::Arc;

use super::block::{Files, Resolved, RichBlock, for_each_text};
use super::rich_text::push_plain;
use crate::enums::ParseMode;
use crate::parser;
use crate::raw::{base, types};
use crate::types::{Document, Photo};

/// The rich content of a message: a list of blocks plus the photos and documents they use.
///
/// Read the content with [`blocks`](RichMessage::blocks) or flatten it with [`text`](RichMessage::text).
#[derive(Debug, Clone, PartialEq)]
pub struct RichMessage {
    /// Whether the message is laid out right to left.
    pub rtl: bool,
    /// Whether Telegram sent only part of the message.
    /// [`Client::get_rich_message`](crate::Client::get_rich_message) fetches all of it.
    pub part: bool,
    pub(crate) raw: Box<types::RichMessage>,
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
            base::InputRichMessage::InputRichMessageHTML(m) => (parser::parse(&m.html, ParseMode::Html).text, None),
            base::InputRichMessage::InputRichMessageMarkdown(m) => {
                (parser::parse(&m.markdown, ParseMode::Markdown).text, None)
            }
        }
    }

    /// Returns the top-level blocks, each able to read its own kind and the photos and documents of this message.
    pub fn blocks(&self) -> Vec<RichBlock> {
        let source = Arc::new(Resolved { photos: self.raw.photos.clone(), documents: self.raw.documents.clone() });
        let files = Files { source: Some(source), ..Files::default() };
        self.raw.blocks.iter().map(|block| RichBlock { block: block.clone(), files: files.clone() }).collect()
    }

    /// Looks up a photo of this message by id. `None` if the message doesn't carry it.
    pub fn photo(&self, id: i64) -> Option<Photo> {
        let photo = self.raw.photos.iter().find(|p| match p {
            base::Photo::Photo(p) => p.id == id,
            base::Photo::PhotoEmpty(p) => p.id == id,
        });
        photo.and_then(Photo::parse)
    }

    /// Looks up a document of this message by id. `None` if the message doesn't carry it.
    pub fn document(&self, id: i64) -> Option<Document> {
        let document = self.raw.documents.iter().find(|d| match d {
            base::Document::Document(d) => d.id == id,
            base::Document::DocumentEmpty(d) => d.id == id,
        });
        document.and_then(Document::parse)
    }

    /// Returns the message as plain text without formatting.
    ///
    /// Each piece of text, including captions and text in nested blocks, goes on its own line. Empty pieces are
    /// skipped.
    pub fn text(&self) -> String {
        RichMessage::plain_text(&self.raw)
    }

    pub(crate) fn plain_text(raw: &types::RichMessage) -> String {
        let mut out = String::new();
        for block in &raw.blocks {
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

#[cfg(test)]
mod tests {
    use super::RichMessage;
    use crate::Client;
    use crate::raw::{base, types};
    use crate::types::{InputRichMessage, RichBlock, RichBlockKind, RichListItem, Text};

    #[tokio::test]
    async fn sent_blocks_read_back_equal() {
        let built = vec![
            RichBlock::heading(2, "Title"),
            RichBlock::paragraph(Text::new().plain("a ").bold("b")),
            RichBlock::ordered_list([RichListItem::new("one"), RichListItem::checkbox("two", true)]),
            RichBlock::details("more", [RichBlock::divider(), RichBlock::math("x^2")]).open(),
            RichBlock::pullquote("q").credit("me"),
        ];
        let input = InputRichMessage::blocks(built.clone()).rtl();
        let raw = input.write(&Client::offline()).await.unwrap();
        let (_, rich) = RichMessage::sent(raw);
        let rich = rich.expect("block content comes back as blocks");
        assert!(rich.rtl);

        let read = rich.blocks();
        assert_eq!(read.len(), built.len());
        for (read, built) in read.iter().zip(&built) {
            assert_eq!(read.block, built.block);
            assert_eq!(std::mem::discriminant(&read.kind()), std::mem::discriminant(&built.kind()));
        }
        assert!(matches!(read[0].kind(), RichBlockKind::Heading { level: 2, .. }));
        let RichBlockKind::Details { blocks, is_open, .. } = read[3].kind() else { panic!("not details") };
        assert!(is_open);
        assert!(matches!(blocks[1].kind(), RichBlockKind::Math(source) if source == "x^2"));
    }

    #[test]
    fn blocks_of_a_received_message_read_its_photos() {
        let size = types::PhotoSize { r#type: "x".into(), w: 10, h: 20, size: 30 };
        let photo = base::Photo::Photo(Box::new(types::Photo {
            has_stickers: false,
            id: 7,
            access_hash: 2,
            file_reference: vec![3],
            date: 4,
            sizes: vec![base::PhotoSize::PhotoSize(Box::new(size))],
            video_sizes: None,
            dc_id: 2,
        }));
        let caption = base::PageCaption::PageCaption(Box::new(types::PageCaption {
            text: base::RichText::TextEmpty,
            credit: base::RichText::TextEmpty,
        }));
        let block = types::PageBlockPhoto { spoiler: false, photo_id: 7, caption, url: None, webpage_id: None };
        let raw = types::RichMessage {
            rtl: false,
            part: false,
            blocks: vec![base::PageBlock::PageBlockPhoto(Box::new(block))],
            photos: vec![photo],
            documents: vec![],
        };
        let rich = RichMessage::parse(base::RichMessage::RichMessage(Box::new(raw)));
        let blocks = rich.blocks();
        let RichBlockKind::Photo { photo, caption, .. } = blocks[0].kind() else { panic!("not a photo") };
        assert_eq!((photo.map(|p| p.id), rich.photo(7).map(|p| p.id), caption.is_none()), (Some(7), Some(7), true));
    }
}
