use std::sync::Arc;

use super::block::{
    Align, Files, Resolved, RichBlock, RichButton, RichButtonStyle, RichListItem, RichTableCell, VAlign,
};
use crate::raw::{base, types};
use crate::types::{
    Animation, Audio, Document, InlineKeyboardButton, Location, Media, Photo, ReplyMarkup, RichText, Video, Voice,
};

/// The caption of a media block, a collage or a slideshow.
#[derive(Debug, Clone, PartialEq)]
pub struct RichBlockCaption {
    /// The caption text. Empty when only a credit is set.
    pub text: RichText,
    /// Who or what the block is credited to. `None` when the caption has no credit.
    pub credit: Option<RichText>,
}

/// What a block of a received rich message is.
///
/// A media block keeps its kind when the message doesn't carry its file (as in a message you just sent): the file is
/// `None`. `Unsupported` is only for the legacy page blocks that have no friendly form.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum RichBlockKind {
    /// A paragraph of text.
    Paragraph(RichText),
    /// A heading.
    Heading {
        /// Heading level, from 1 for the largest to 6.
        level: u8,
        /// Text of the heading.
        text: RichText,
    },
    /// A preformatted code block. `language` is empty when the block names none.
    Preformatted {
        /// The code text.
        text: RichText,
        /// Language name, empty when none is given.
        language: String,
    },
    /// A footer line.
    Footer(RichText),
    /// A horizontal divider.
    Divider,
    /// A math block holding the formula source.
    Math(String),
    /// A named anchor holding the anchor name.
    Anchor(String),
    /// A thinking block.
    Thinking(RichText),
    /// A bulleted or numbered list.
    List(Vec<RichListItem>),
    /// A quote made of blocks. A plain text quote is read as one paragraph block. `credit` is `None` when the
    /// quote has no attribution.
    BlockQuote {
        /// Blocks inside the quote.
        blocks: Vec<RichBlock>,
        /// Attribution of the quote.
        credit: Option<RichText>,
    },
    /// A text quote that stays collapsed until the reader expands it. `credit` is `None` when the quote has no
    /// attribution.
    ExpandableBlockQuote {
        /// Text of the quote.
        text: RichText,
        /// Attribution of the quote.
        credit: Option<RichText>,
    },
    /// A highlighted excerpt. `credit` is `None` when it has no attribution.
    PullQuote {
        /// Text of the excerpt.
        text: RichText,
        /// Attribution of the excerpt.
        credit: Option<RichText>,
    },
    /// A collage of media blocks. `caption` is `None` when it has neither text nor credit.
    Collage {
        /// Media blocks in the collage.
        blocks: Vec<RichBlock>,
        /// Caption of the collage.
        caption: Option<RichBlockCaption>,
    },
    /// A slideshow of media blocks. `caption` is `None` when it has neither text nor credit.
    Slideshow {
        /// Media blocks in the slideshow.
        blocks: Vec<RichBlock>,
        /// Caption of the slideshow.
        caption: Option<RichBlockCaption>,
    },
    /// A table. `title` is `None` when the table has none, and rows without cells are dropped.
    Table {
        /// Title above the table.
        title: Option<RichText>,
        /// Rows of cells, in order.
        rows: Vec<Vec<RichTableCell>>,
        /// Whether the table draws borders.
        is_bordered: bool,
        /// Whether alternate rows are shaded.
        is_striped: bool,
        /// Whether the table uses tighter spacing.
        is_compact: bool,
    },
    /// A collapsible section with a `summary` line.
    Details {
        /// Line that stays visible when the section is collapsed.
        summary: RichText,
        /// Blocks inside the section.
        blocks: Vec<RichBlock>,
        /// Whether the section is shown expanded.
        is_open: bool,
    },
    /// A map. A map whose coordinates can't be read is [`Unsupported`](RichBlockKind::Unsupported) instead.
    Map {
        /// Center of the map.
        location: Location,
        /// Zoom level of the map.
        zoom: i32,
        /// Width of the map frame.
        width: i32,
        /// Height of the map frame.
        height: i32,
        /// Caption of the map.
        caption: Option<RichBlockCaption>,
    },
    /// A row of buttons. `align` is `None` when the row sets no alignment.
    Buttons {
        /// Buttons in the row.
        buttons: Vec<RichButton>,
        /// Alignment of the row.
        align: Option<Align>,
    },
    /// A photo. `photo` is `None` when the message doesn't carry the file.
    Photo {
        /// The photo file.
        photo: Option<Photo>,
        /// Whether the photo is hidden behind a spoiler.
        has_spoiler: bool,
        /// Caption of the photo.
        caption: Option<RichBlockCaption>,
    },
    /// A video. `video` is `None` when the message doesn't carry the file.
    Video {
        /// The video file.
        video: Option<Video>,
        /// Whether the video is hidden behind a spoiler.
        has_spoiler: bool,
        /// Caption of the video.
        caption: Option<RichBlockCaption>,
    },
    /// An animation. Without the file, a video block that autoplays and loops is read as this, with `animation`
    /// set to `None`.
    Animation {
        /// The animation file.
        animation: Option<Animation>,
        /// Whether the animation is hidden behind a spoiler.
        has_spoiler: bool,
        /// Caption of the animation.
        caption: Option<RichBlockCaption>,
    },
    /// An audio file. `audio` is `None` when the message doesn't carry the file.
    Audio {
        /// The audio file.
        audio: Option<Audio>,
        /// Caption of the audio file.
        caption: Option<RichBlockCaption>,
    },
    /// A voice note, read from a video or audio block whose file is a voice note.
    VoiceNote {
        /// The voice note file.
        voice_note: Voice,
        /// Caption of the voice note.
        caption: Option<RichBlockCaption>,
    },
    /// A document. `document` is `None` when the message doesn't carry the file.
    Document {
        /// The document file.
        document: Option<Document>,
        /// Caption of the document.
        caption: Option<RichBlockCaption>,
    },
    /// A legacy page block with no friendly form, or a map whose coordinates can't be read.
    Unsupported,
}

fn text(text: &base::RichText) -> RichText {
    RichText(text.clone())
}

fn credit(text: &base::RichText) -> Option<RichText> {
    (!matches!(text, base::RichText::TextEmpty)).then(|| RichText(text.clone()))
}

fn caption(caption: &base::PageCaption) -> Option<RichBlockCaption> {
    let base::PageCaption::PageCaption(c) = caption;
    let text = RichText(c.text.clone());
    let credit = credit(&c.credit);
    (!text.is_empty() || credit.is_some()).then_some(RichBlockCaption { text, credit })
}

impl RichBlock {
    /// Reads the block into a [`RichBlockKind`]. Media is resolved from the message the block came from.
    pub fn kind(&self) -> RichBlockKind {
        use base::PageBlock as B;
        let blocks = |items: &[base::PageBlock]| items.iter().map(|b| self.child(b)).collect::<Vec<_>>();
        let heading = |level, t: &base::RichText| RichBlockKind::Heading { level, text: text(t) };
        match &self.block {
            B::PageBlockParagraph(b) => RichBlockKind::Paragraph(text(&b.text)),
            B::PageBlockHeading1(b) => heading(1, &b.text),
            B::PageBlockHeading2(b) => heading(2, &b.text),
            B::PageBlockHeading3(b) => heading(3, &b.text),
            B::PageBlockHeading4(b) => heading(4, &b.text),
            B::PageBlockHeading5(b) => heading(5, &b.text),
            B::PageBlockHeading6(b) => heading(6, &b.text),
            B::PageBlockPreformatted(b) => {
                RichBlockKind::Preformatted { text: text(&b.text), language: b.language.clone() }
            }
            B::PageBlockFooter(b) => RichBlockKind::Footer(text(&b.text)),
            B::PageBlockDivider => RichBlockKind::Divider,
            B::PageBlockMath(b) => RichBlockKind::Math(b.source.clone()),
            B::PageBlockAnchor(b) => RichBlockKind::Anchor(b.name.clone()),
            B::PageBlockThinking(b) => RichBlockKind::Thinking(text(&b.text)),
            B::PageBlockList(b) => RichBlockKind::List(b.items.iter().map(|i| self.list_item(i)).collect()),
            B::PageBlockOrderedList(b) => RichBlockKind::List(self.ordered_items(b)),
            B::PageBlockBlockquoteBlocks(b) => {
                RichBlockKind::BlockQuote { blocks: blocks(&b.blocks), credit: credit(&b.caption) }
            }
            B::PageBlockBlockquote(b) if b.collapsed => {
                RichBlockKind::ExpandableBlockQuote { text: text(&b.text), credit: credit(&b.caption) }
            }
            B::PageBlockBlockquote(b) => {
                RichBlockKind::BlockQuote { blocks: vec![self.text_paragraph(&b.text)], credit: credit(&b.caption) }
            }
            B::PageBlockPullquote(b) => RichBlockKind::PullQuote { text: text(&b.text), credit: credit(&b.caption) },
            B::PageBlockCollage(b) => RichBlockKind::Collage { blocks: blocks(&b.items), caption: caption(&b.caption) },
            B::PageBlockSlideshow(b) => {
                RichBlockKind::Slideshow { blocks: blocks(&b.items), caption: caption(&b.caption) }
            }
            B::PageBlockTable(b) => RichBlockKind::Table {
                title: credit(&b.title),
                rows: b
                    .rows
                    .iter()
                    .map(|base::PageTableRow::PageTableRow(row)| row.cells.iter().cloned().map(RichTableCell).collect())
                    .filter(|cells: &Vec<RichTableCell>| !cells.is_empty())
                    .collect(),
                is_bordered: b.bordered,
                is_striped: b.striped,
                is_compact: b.compact,
            },
            B::PageBlockDetails(b) => {
                RichBlockKind::Details { summary: text(&b.title), blocks: blocks(&b.blocks), is_open: b.open }
            }
            B::PageBlockMap(b) => match Location::parse(&b.geo) {
                Some(location) => {
                    RichBlockKind::Map { location, zoom: b.zoom, width: b.w, height: b.h, caption: caption(&b.caption) }
                }
                None => RichBlockKind::Unsupported,
            },
            B::PageBlockButtonRow(b) => {
                let align = if b.align_left {
                    Some(Align::Left)
                } else if b.align_center {
                    Some(Align::Center)
                } else {
                    b.align_right.then_some(Align::Right)
                };
                let buttons = b.buttons.iter().map(RichButton::read).collect();
                RichBlockKind::Buttons { buttons, align }
            }
            B::PageBlockPhoto(b) => RichBlockKind::Photo {
                photo: self.photo_of(b.photo_id),
                has_spoiler: b.spoiler,
                caption: caption(&b.caption),
            },
            B::PageBlockVideo(b) => {
                let caption = caption(&b.caption);
                match self.media(b.video_id) {
                    Some(Media::Animation(animation)) => {
                        RichBlockKind::Animation { animation: Some(animation), has_spoiler: b.spoiler, caption }
                    }
                    Some(Media::Video(video)) => {
                        RichBlockKind::Video { video: Some(video), has_spoiler: b.spoiler, caption }
                    }
                    Some(Media::Voice(voice_note)) => RichBlockKind::VoiceNote { voice_note, caption },
                    Some(Media::Audio(audio)) => RichBlockKind::Audio { audio: Some(audio), caption },
                    // Without the file there is only the player's hint: a looping autoplay is an animation.
                    _ if b.autoplay && b.r#loop => {
                        RichBlockKind::Animation { animation: None, has_spoiler: b.spoiler, caption }
                    }
                    _ => RichBlockKind::Video { video: None, has_spoiler: b.spoiler, caption },
                }
            }
            B::PageBlockAudio(b) => {
                let caption = caption(&b.caption);
                match self.media(b.audio_id) {
                    Some(Media::Voice(voice_note)) => RichBlockKind::VoiceNote { voice_note, caption },
                    Some(Media::Audio(audio)) => RichBlockKind::Audio { audio: Some(audio), caption },
                    _ => RichBlockKind::Audio { audio: None, caption },
                }
            }
            B::PageBlockDocument(b) => {
                let document = self.source().and_then(|s| find_document(s, b.document_id)).and_then(Document::parse);
                RichBlockKind::Document { document, caption: caption(&b.caption) }
            }
            B::PageBlockUnsupported
            | B::PageBlockTitle(_)
            | B::PageBlockSubtitle(_)
            | B::PageBlockAuthorDate(_)
            | B::PageBlockHeader(_)
            | B::PageBlockSubheader(_)
            | B::PageBlockCover(_)
            | B::PageBlockEmbed(_)
            | B::PageBlockEmbedPost(_)
            | B::PageBlockChannel(_)
            | B::PageBlockKicker(_)
            | B::PageBlockRelatedArticles(_)
            | B::InputPageBlockMap(_) => RichBlockKind::Unsupported,
        }
    }

    fn source(&self) -> Option<&Arc<Resolved>> {
        self.files.source.as_ref()
    }

    fn child(&self, block: &base::PageBlock) -> RichBlock {
        RichBlock { block: block.clone(), files: Files { source: self.files.source.clone(), ..Files::default() } }
    }

    fn text_paragraph(&self, text: &base::RichText) -> RichBlock {
        let paragraph = types::PageBlockParagraph { text: text.clone() };
        self.child(&base::PageBlock::PageBlockParagraph(Box::new(paragraph)))
    }

    fn photo_of(&self, id: i64) -> Option<Photo> {
        let source = self.source()?;
        let found = source.photos.iter().find(|p| match p {
            base::Photo::Photo(p) => p.id == id,
            base::Photo::PhotoEmpty(p) => p.id == id,
        });
        found.and_then(Photo::parse)
    }

    fn media(&self, id: i64) -> Option<Media> {
        Media::from_document(find_document(self.source()?, id)?)
    }

    fn list_item(&self, item: &base::PageListItem) -> RichListItem {
        let (checkbox, checked, blocks) = match item {
            base::PageListItem::PageListItemText(i) => (i.checkbox, i.checked, vec![self.text_paragraph(&i.text)]),
            base::PageListItem::PageListItemBlocks(i) => {
                (i.checkbox, i.checked, i.blocks.iter().map(|b| self.child(b)).collect())
            }
        };
        RichListItem { checkbox, checked, blocks, label: "•".into(), value: None, numbering: None }
    }

    fn ordered_items(&self, list: &types::PageBlockOrderedList) -> Vec<RichListItem> {
        let list_type = numbering(list.r#type.as_deref());
        let mut next = list.start.unwrap_or(1);
        let step = if list.reversed { -1 } else { 1 };
        list.items
            .iter()
            .map(|item| {
                let (checkbox, checked, num, value, own_type, blocks) = match item {
                    base::PageListOrderedItem::PageListOrderedItemText(i) => {
                        (i.checkbox, i.checked, &i.num, i.value, &i.r#type, vec![self.text_paragraph(&i.text)])
                    }
                    base::PageListOrderedItem::PageListOrderedItemBlocks(i) => {
                        let blocks = i.blocks.iter().map(|b| self.child(b)).collect();
                        (i.checkbox, i.checked, &i.num, i.value, &i.r#type, blocks)
                    }
                };
                let value = value.unwrap_or(next);
                next = value.saturating_add(step);
                let kind = own_type.as_deref().map_or(list_type, |t| numbering(Some(t)));
                let label = match num {
                    Some(num) => format!("{num}."),
                    None => ordered_label(value, kind),
                };
                RichListItem { checkbox, checked, blocks, label, value: Some(value), numbering: Some(kind.into()) }
            })
            .collect()
    }
}

fn find_document(source: &Resolved, id: i64) -> Option<&base::Document> {
    source.documents.iter().find(|d| match d {
        base::Document::Document(d) => d.id == id,
        base::Document::DocumentEmpty(d) => d.id == id,
    })
}

/// A numbering type Telegram doesn't define counts as decimal.
fn numbering(kind: Option<&str>) -> &'static str {
    match kind {
        Some("a") => "a",
        Some("A") => "A",
        Some("i") => "i",
        Some("I") => "I",
        _ => "1",
    }
}

fn ordered_label(num: i32, kind: &str) -> String {
    if num > 0 && (kind == "a" || kind == "A") {
        let base = if kind == "A" { b'A' } else { b'a' };
        let (mut rest, mut letters) = (num as u32, Vec::new());
        while rest > 0 {
            rest -= 1;
            letters.push(base + (rest % 26) as u8);
            rest /= 26;
        }
        letters.reverse();
        return format!("{}.", String::from_utf8_lossy(&letters));
    }
    if num > 0 && num < 4000 && (kind == "i" || kind == "I") {
        const NUMERALS: [(i32, &str); 13] = [
            (1000, "M"),
            (900, "CM"),
            (500, "D"),
            (400, "CD"),
            (100, "C"),
            (90, "XC"),
            (50, "L"),
            (40, "XL"),
            (10, "X"),
            (9, "IX"),
            (5, "V"),
            (4, "IV"),
            (1, "I"),
        ];
        let (mut rest, mut roman) = (num, String::new());
        for (value, numeral) in NUMERALS {
            while rest >= value {
                roman.push_str(numeral);
                rest -= value;
            }
        }
        return format!("{}.", if kind == "i" { roman.to_lowercase() } else { roman });
    }
    format!("{num}.")
}

impl RichListItem {
    /// Returns the label shown before the item: a bullet, or the item's number or letter. Empty for an item you
    /// built.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the blocks that make up the item.
    pub fn content(&self) -> &[RichBlock] {
        &self.blocks
    }

    /// Returns `true` if the item shows a checkbox.
    pub fn has_checkbox(&self) -> bool {
        self.checkbox
    }

    /// Returns `true` if the item's checkbox is ticked.
    pub fn is_checked(&self) -> bool {
        self.checked
    }

    /// Returns the item's number in an ordered list. `None` for a bulleted list or an item you built.
    pub fn value(&self) -> Option<i32> {
        self.value
    }

    /// Returns how an ordered list writes its numbers: `"1"`, `"a"`, `"A"`, `"i"` or `"I"`. `None` for a
    /// bulleted list or an item you built.
    pub fn numbering(&self) -> Option<&str> {
        self.numbering.as_deref()
    }
}

impl RichTableCell {
    /// Returns the cell text. `None` when the cell has no text.
    pub fn text(&self) -> Option<RichText> {
        let base::PageTableCell::PageTableCell(c) = &self.0;
        c.text.as_ref().map(text)
    }

    /// Returns `true` for a header cell.
    pub fn is_header(&self) -> bool {
        let base::PageTableCell::PageTableCell(c) = &self.0;
        c.header
    }

    /// Returns how many columns the cell spans, at least 1.
    pub fn column_span(&self) -> i32 {
        let base::PageTableCell::PageTableCell(c) = &self.0;
        c.colspan.unwrap_or(1).max(1)
    }

    /// Returns how many rows the cell spans, at least 1.
    pub fn row_span(&self) -> i32 {
        let base::PageTableCell::PageTableCell(c) = &self.0;
        c.rowspan.unwrap_or(1).max(1)
    }

    /// Returns the horizontal alignment of the cell text, [`Align::Left`] unless set otherwise.
    pub fn horizontal_alignment(&self) -> Align {
        let base::PageTableCell::PageTableCell(c) = &self.0;
        if c.align_center {
            Align::Center
        } else if c.align_right {
            Align::Right
        } else {
            Align::Left
        }
    }

    /// Returns the vertical alignment of the cell text, [`VAlign::Top`] unless set otherwise.
    pub fn vertical_alignment(&self) -> VAlign {
        let base::PageTableCell::PageTableCell(c) = &self.0;
        if c.valign_middle {
            VAlign::Middle
        } else if c.valign_bottom {
            VAlign::Bottom
        } else {
            VAlign::Top
        }
    }
}

impl RichButton {
    pub(super) fn read(button: &base::PageButton) -> RichButton {
        let base::PageButton::PageButton(b) = button;
        RichButton { text: text(&b.text).text(), kind: b.r#type.clone(), style: read_style(b.style.as_ref()) }
    }

    /// Returns the button label.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the button's color style, [`RichButtonStyle::Default`] when none is set.
    pub fn button_style(&self) -> RichButtonStyle {
        self.style
    }

    /// Returns what the button does. `None` for a button the keyboard parser can't read, such as a callback
    /// whose data isn't UTF-8.
    pub fn action(&self) -> Option<InlineKeyboardButton> {
        button_action(&self.kind, &self.text)
    }
}

pub(super) fn read_style(style: Option<&base::RichButtonStyle>) -> RichButtonStyle {
    let Some(base::RichButtonStyle::RichButtonStyle(s)) = style else { return RichButtonStyle::Default };
    if s.bg_primary {
        RichButtonStyle::Primary
    } else if s.bg_danger {
        RichButtonStyle::Danger
    } else if s.bg_success {
        RichButtonStyle::Success
    } else if s.link {
        RichButtonStyle::Link
    } else {
        RichButtonStyle::Default
    }
}

/// Reads the button through the keyboard parser, so rich buttons and keyboard buttons agree on what is usable.
pub(super) fn button_action(kind: &base::InlineButtonType, text: &str) -> Option<InlineKeyboardButton> {
    let button = types::KeyboardInlineButton { style: None, text: text.into(), r#type: kind.clone() };
    let row = types::KeyboardInlineButtonRow {
        buttons: vec![base::KeyboardInlineButton::KeyboardInlineButton(Box::new(button))],
    };
    let markup = types::ReplyInlineMarkup {
        force_reply: false,
        rows: vec![base::KeyboardInlineButtonRow::KeyboardInlineButtonRow(Box::new(row))],
    };
    match ReplyMarkup::parse(&base::ReplyMarkup::ReplyInlineMarkup(Box::new(markup)))? {
        ReplyMarkup::Inline(markup) => markup.rows.into_iter().next()?.into_iter().next(),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::super::block::{Files, Resolved};
    use super::{RichBlock, RichBlockKind as K, ordered_label};
    use crate::raw::{base, types};
    use crate::types::{Align, RichButtonStyle, VAlign};

    fn plain(text: &str) -> base::RichText {
        base::RichText::TextPlain(Box::new(types::TextPlain { text: text.into() }))
    }

    fn block(block: base::PageBlock) -> RichBlock {
        RichBlock { block, files: Files::default() }
    }

    fn caption(text: &str, credit: &str) -> base::PageCaption {
        let credit = if credit.is_empty() { base::RichText::TextEmpty } else { plain(credit) };
        base::PageCaption::PageCaption(Box::new(types::PageCaption { text: plain(text), credit }))
    }

    fn geo() -> base::GeoPoint {
        base::GeoPoint::GeoPoint(Box::new(types::GeoPoint {
            long: 2.0,
            lat: 1.0,
            access_hash: 0,
            accuracy_radius: None,
        }))
    }

    #[test]
    fn every_page_block_has_a_kind() {
        use base::PageBlock as B;
        let t = || plain("x");
        let paragraph = || B::PageBlockParagraph(Box::new(types::PageBlockParagraph { text: t() }));
        let supported = vec![
            (paragraph(), "Paragraph"),
            (B::PageBlockHeading1(Box::new(types::PageBlockHeading1 { text: t() })), "Heading"),
            (B::PageBlockHeading6(Box::new(types::PageBlockHeading6 { text: t() })), "Heading"),
            (
                B::PageBlockPreformatted(Box::new(types::PageBlockPreformatted { text: t(), language: "rs".into() })),
                "Pre",
            ),
            (B::PageBlockFooter(Box::new(types::PageBlockFooter { text: t() })), "Footer"),
            (B::PageBlockDivider, "Divider"),
            (B::PageBlockMath(Box::new(types::PageBlockMath { source: "x".into() })), "Math"),
            (B::PageBlockAnchor(Box::new(types::PageBlockAnchor { name: "a".into() })), "Anchor"),
            (B::PageBlockThinking(Box::new(types::PageBlockThinking { text: t() })), "Thinking"),
            (B::PageBlockList(Box::new(types::PageBlockList { items: vec![] })), "List"),
            (
                B::PageBlockOrderedList(Box::new(types::PageBlockOrderedList {
                    reversed: false,
                    items: vec![],
                    start: None,
                    r#type: None,
                })),
                "List",
            ),
            (
                B::PageBlockBlockquote(Box::new(types::PageBlockBlockquote {
                    collapsed: false,
                    text: t(),
                    caption: base::RichText::TextEmpty,
                })),
                "BlockQuote",
            ),
            (
                B::PageBlockBlockquote(Box::new(types::PageBlockBlockquote {
                    collapsed: true,
                    text: t(),
                    caption: base::RichText::TextEmpty,
                })),
                "ExpandableBlockQuote",
            ),
            (
                B::PageBlockBlockquoteBlocks(Box::new(types::PageBlockBlockquoteBlocks {
                    blocks: vec![paragraph()],
                    caption: t(),
                })),
                "BlockQuote",
            ),
            (
                B::PageBlockPullquote(Box::new(types::PageBlockPullquote {
                    text: t(),
                    caption: base::RichText::TextEmpty,
                })),
                "PullQuote",
            ),
            (
                B::PageBlockCollage(Box::new(types::PageBlockCollage { items: vec![], caption: caption("", "") })),
                "Collage",
            ),
            (
                B::PageBlockSlideshow(Box::new(types::PageBlockSlideshow { items: vec![], caption: caption("", "") })),
                "Slideshow",
            ),
            (
                B::PageBlockTable(Box::new(types::PageBlockTable {
                    bordered: true,
                    striped: false,
                    compact: false,
                    title: base::RichText::TextEmpty,
                    rows: vec![],
                })),
                "Table",
            ),
            (
                B::PageBlockDetails(Box::new(types::PageBlockDetails { open: true, blocks: vec![], title: t() })),
                "Details",
            ),
            (
                B::PageBlockMap(Box::new(types::PageBlockMap {
                    geo: geo(),
                    zoom: 15,
                    w: 100,
                    h: 50,
                    caption: caption("", ""),
                })),
                "Map",
            ),
            (
                B::PageBlockButtonRow(Box::new(types::PageBlockButtonRow {
                    align_left: false,
                    align_center: true,
                    align_right: false,
                    buttons: vec![],
                })),
                "Buttons",
            ),
        ];
        for (raw, expected) in supported {
            let kind = block(raw).kind();
            let name = format!("{kind:?}");
            assert!(name.starts_with(expected), "{expected} block read as {name}");
        }

        let unsupported = vec![
            B::PageBlockUnsupported,
            B::PageBlockTitle(Box::new(types::PageBlockTitle { text: t() })),
            B::PageBlockSubtitle(Box::new(types::PageBlockSubtitle { text: t() })),
            B::PageBlockKicker(Box::new(types::PageBlockKicker { text: t() })),
            B::PageBlockHeader(Box::new(types::PageBlockHeader { text: t() })),
            B::PageBlockSubheader(Box::new(types::PageBlockSubheader { text: t() })),
            B::PageBlockCover(Box::new(types::PageBlockCover { cover: paragraph() })),
        ];
        for raw in unsupported {
            let kind = block(raw).kind();
            assert!(matches!(kind, K::Unsupported), "read as {kind:?}");
        }
    }

    #[test]
    fn headings_quotes_and_captions_keep_their_parts() {
        use base::PageBlock as B;
        let heading = block(B::PageBlockHeading3(Box::new(types::PageBlockHeading3 { text: plain("h") }))).kind();
        assert!(matches!(heading, K::Heading { level: 3, .. }));

        let quote = types::PageBlockBlockquote { collapsed: false, text: plain("q"), caption: plain("me") };
        let K::BlockQuote { blocks, credit } = block(B::PageBlockBlockquote(Box::new(quote))).kind() else {
            panic!("not a quote")
        };
        assert!(matches!(blocks[0].kind(), K::Paragraph(t) if t.text() == "q"));
        assert_eq!(credit.map(|c| c.text()), Some("me".into()));

        let collage = types::PageBlockCollage { items: vec![], caption: caption("pics", "cam") };
        let K::Collage { caption, .. } = block(B::PageBlockCollage(Box::new(collage))).kind() else {
            panic!("not a collage")
        };
        let caption = caption.expect("a caption");
        assert_eq!((caption.text.text(), caption.credit.map(|c| c.text())), ("pics".into(), Some("cam".into())));
    }

    #[test]
    fn tables_read_cells_and_flags() {
        use base::PageBlock as B;
        let cell = |text: Option<&str>, header, colspan| {
            base::PageTableCell::PageTableCell(Box::new(types::PageTableCell {
                header,
                align_center: false,
                align_right: true,
                valign_middle: false,
                valign_bottom: true,
                text: text.map(plain),
                colspan,
                rowspan: Some(0),
            }))
        };
        let row = |cells| base::PageTableRow::PageTableRow(Box::new(types::PageTableRow { cells }));
        let table = types::PageBlockTable {
            bordered: true,
            striped: true,
            compact: false,
            title: plain("t"),
            rows: vec![row(vec![cell(Some("a"), true, Some(2)), cell(None, false, None)]), row(vec![])],
        };
        let K::Table { title, rows, is_bordered, is_striped, is_compact } =
            block(B::PageBlockTable(Box::new(table))).kind()
        else {
            panic!("not a table")
        };
        assert_eq!(
            (title.map(|t| t.text()), is_bordered, is_striped, is_compact),
            (Some("t".into()), true, true, false)
        );
        assert_eq!(rows.len(), 1, "an empty row is dropped");
        let (a, hidden) = (&rows[0][0], &rows[0][1]);
        assert_eq!(
            (a.text().map(|t| t.text()), a.is_header(), a.column_span(), a.row_span()),
            (Some("a".into()), true, 2, 1)
        );
        assert_eq!((a.horizontal_alignment(), a.vertical_alignment()), (Align::Right, VAlign::Bottom));
        assert!(hidden.text().is_none() && !hidden.is_header());
    }

    #[test]
    fn lists_read_labels_values_and_checkboxes() {
        use base::PageBlock as B;
        let item = |num: Option<&str>, value, kind: Option<&str>| {
            base::PageListOrderedItem::PageListOrderedItemText(Box::new(types::PageListOrderedItemText {
                checkbox: true,
                checked: true,
                num: num.map(Into::into),
                text: plain("i"),
                value,
                r#type: kind.map(Into::into),
            }))
        };
        let list = types::PageBlockOrderedList {
            reversed: true,
            items: vec![
                item(None, None, None),
                item(None, None, None),
                item(Some("7"), None, None),
                item(None, Some(2), Some("A")),
            ],
            start: Some(5),
            r#type: Some("i".into()),
        };
        let K::List(items) = block(B::PageBlockOrderedList(Box::new(list))).kind() else { panic!("not a list") };
        let read: Vec<_> = items.iter().map(|i| (i.label(), i.value(), i.numbering())).collect();
        assert_eq!(
            read,
            [
                ("v.", Some(5), Some("i")),
                ("iv.", Some(4), Some("i")),
                ("7.", Some(3), Some("i")),
                ("B.", Some(2), Some("A"))
            ]
        );
        assert!(items[0].has_checkbox() && items[0].is_checked());
        assert!(matches!(items[0].content()[0].kind(), K::Paragraph(_)));

        let bullet = base::PageListItem::PageListItemBlocks(Box::new(types::PageListItemBlocks {
            checkbox: false,
            checked: false,
            blocks: vec![B::PageBlockDivider],
        }));
        let list = types::PageBlockList { items: vec![bullet] };
        let K::List(items) = block(B::PageBlockList(Box::new(list))).kind() else { panic!("not a list") };
        assert_eq!((items[0].label(), items[0].value(), items[0].numbering()), ("•", None, None));
        assert!(matches!(items[0].content()[0].kind(), K::Divider));
    }

    #[test]
    fn ordered_labels_follow_the_numbering_type() {
        assert_eq!(ordered_label(1, "1"), "1.");
        assert_eq!(ordered_label(27, "a"), "aa.");
        assert_eq!(ordered_label(26, "A"), "Z.");
        assert_eq!(ordered_label(1994, "I"), "MCMXCIV.");
        assert_eq!(ordered_label(4000, "I"), "4000.");
        assert_eq!(ordered_label(0, "a"), "0.");
        assert_eq!(ordered_label(-3, "i"), "-3.");
    }

    #[test]
    fn buttons_read_text_style_and_action() {
        use base::PageBlock as B;
        let url = types::InlineButtonTypeUrl { url: "https://t.me".into() };
        let style = types::RichButtonStyle { bg_primary: true, bg_danger: false, bg_success: false, link: false };
        let go = base::PageButton::PageButton(Box::new(types::PageButton {
            text: plain("Go"),
            r#type: base::InlineButtonType::InlineButtonTypeUrl(Box::new(url)),
            style: Some(base::RichButtonStyle::RichButtonStyle(style)),
        }));
        let pay = base::PageButton::PageButton(Box::new(types::PageButton {
            text: plain("Pay"),
            r#type: base::InlineButtonType::InlineButtonTypeBuy,
            style: None,
        }));
        let unreadable = base::PageButton::PageButton(Box::new(types::PageButton {
            text: plain("Odd"),
            r#type: base::InlineButtonType::InlineButtonTypeCallback(Box::new(types::InlineButtonTypeCallback {
                requires_password: false,
                data: vec![0xff, 0xfe],
            })),
            style: None,
        }));
        let row = types::PageBlockButtonRow {
            align_left: false,
            align_center: false,
            align_right: true,
            buttons: vec![go, pay, unreadable],
        };
        let K::Buttons { buttons, align } = block(B::PageBlockButtonRow(Box::new(row))).kind() else {
            panic!("not buttons")
        };
        assert_eq!(align, Some(Align::Right));
        assert_eq!((buttons[0].text(), buttons[0].button_style()), ("Go", RichButtonStyle::Primary));
        assert_eq!(buttons[0].action(), Some(crate::types::InlineKeyboardButton::url("Go", "https://t.me")));
        // Pay buttons are read like any keyboard button; only unreadable callback data gives no action.
        assert_eq!(buttons[1].action(), Some(crate::types::InlineKeyboardButton::pay("Pay")));
        assert_eq!(buttons[1].button_style(), RichButtonStyle::Default);
        assert!(buttons[2].action().is_none());
    }

    fn document(id: i64, attributes: Vec<base::DocumentAttribute>) -> base::Document {
        base::Document::Document(Box::new(types::Document {
            id,
            access_hash: 2,
            file_reference: vec![3],
            date: 4,
            mime_type: "application/octet-stream".into(),
            size: 5,
            thumbs: None,
            video_thumbs: None,
            dc_id: 2,
            attributes,
        }))
    }

    fn video_attribute() -> base::DocumentAttribute {
        base::DocumentAttribute::DocumentAttributeVideo(Box::new(types::DocumentAttributeVideo {
            round_message: false,
            supports_streaming: true,
            nosound: false,
            duration: 3.0,
            w: 64,
            h: 48,
            preload_prefix_size: None,
            video_start_ts: None,
            video_codec: None,
        }))
    }

    fn audio_attribute(voice: bool) -> base::DocumentAttribute {
        base::DocumentAttribute::DocumentAttributeAudio(Box::new(types::DocumentAttributeAudio {
            voice,
            duration: 3,
            title: None,
            performer: None,
            waveform: None,
        }))
    }

    fn photo(id: i64) -> base::Photo {
        let size = types::PhotoSize { r#type: "x".into(), w: 10, h: 20, size: 30 };
        base::Photo::Photo(Box::new(types::Photo {
            has_stickers: false,
            id,
            access_hash: 2,
            file_reference: vec![3],
            date: 4,
            sizes: vec![base::PhotoSize::PhotoSize(Box::new(size))],
            video_sizes: None,
            dc_id: 2,
        }))
    }

    #[test]
    fn blocks_resolve_their_photos() {
        use base::PageBlock as B;
        let photos = vec![photo(1)];
        let documents = vec![
            document(10, vec![video_attribute()]),
            document(11, vec![video_attribute(), base::DocumentAttribute::DocumentAttributeAnimated]),
            document(12, vec![audio_attribute(false)]),
            document(13, vec![audio_attribute(true)]),
            document(14, vec![]),
        ];
        let source = Some(Arc::new(Resolved { photos, documents }));
        let read = |raw: base::PageBlock| {
            RichBlock { block: raw, files: Files { source: source.clone(), ..Files::default() } }.kind()
        };
        let video = |id| {
            B::PageBlockVideo(Box::new(types::PageBlockVideo {
                autoplay: false,
                r#loop: false,
                spoiler: true,
                video_id: id,
                caption: caption("c", ""),
            }))
        };
        let audio = |id| B::PageBlockAudio(Box::new(types::PageBlockAudio { audio_id: id, caption: caption("", "") }));

        let photo_block = types::PageBlockPhoto {
            spoiler: true,
            photo_id: 1,
            caption: caption("look", ""),
            url: None,
            webpage_id: None,
        };
        let K::Photo { photo, has_spoiler, caption } = read(B::PageBlockPhoto(Box::new(photo_block))) else {
            panic!("not a photo")
        };
        let photo = photo.expect("the photo is in the message");
        assert_eq!((photo.id, photo.width, photo.height, has_spoiler), (1, 10, 20, true));
        assert_eq!(caption.map(|c| c.text.text()), Some("look".into()));

        assert!(matches!(read(video(10)), K::Video { video: Some(video), has_spoiler: true, .. } if video.id == 10));
        assert!(matches!(read(video(11)), K::Animation { animation: Some(animation), .. } if animation.id == 11));
        assert!(matches!(read(audio(12)), K::Audio { audio: Some(audio), .. } if audio.id == 12));
        assert!(matches!(read(audio(13)), K::VoiceNote { voice_note, .. } if voice_note.id == 13));
        let document = types::PageBlockDocument { document_id: 14, caption: caption_of("file") };
        assert!(
            matches!(read(B::PageBlockDocument(Box::new(document))), K::Document { document: Some(document), .. } if document.id == 14)
        );
        assert!(matches!(read(video(99)), K::Video { video: None, .. }));
        assert!(matches!(read(audio(10)), K::Audio { audio: None, .. }), "a plain video is not audio");

        // Nested blocks keep reading from the same message.
        let collage = types::PageBlockCollage {
            items: vec![B::PageBlockPhoto(Box::new(types::PageBlockPhoto {
                spoiler: false,
                photo_id: 1,
                caption: caption_of(""),
                url: None,
                webpage_id: None,
            }))],
            caption: caption_of(""),
        };
        let K::Collage { blocks, .. } = read(B::PageBlockCollage(Box::new(collage))) else { panic!("not a collage") };
        assert!(matches!(blocks[0].kind(), K::Photo { .. }));
    }

    #[test]
    fn media_blocks_without_their_file_keep_their_kind() {
        use base::PageBlock as B;
        let video = |autoplay, r#loop| {
            block(B::PageBlockVideo(Box::new(types::PageBlockVideo {
                autoplay,
                r#loop,
                spoiler: true,
                video_id: 1,
                caption: caption("", ""),
            })))
            .kind()
        };
        let photo = types::PageBlockPhoto {
            spoiler: true,
            photo_id: 1,
            caption: caption("look", ""),
            url: None,
            webpage_id: None,
        };
        let K::Photo { photo, has_spoiler, caption } = block(B::PageBlockPhoto(Box::new(photo))).kind() else {
            panic!("not a photo")
        };
        assert!(photo.is_none() && has_spoiler);
        assert_eq!(caption.map(|c| c.text.text()), Some("look".into()));
        assert!(matches!(video(false, false), K::Video { video: None, has_spoiler: true, .. }));
        assert!(matches!(video(true, true), K::Animation { animation: None, .. }));
        let audio = types::PageBlockAudio { audio_id: 1, caption: caption_of("") };
        assert!(matches!(block(B::PageBlockAudio(Box::new(audio))).kind(), K::Audio { audio: None, .. }));
        let document = types::PageBlockDocument { document_id: 1, caption: caption_of("") };
        assert!(matches!(block(B::PageBlockDocument(Box::new(document))).kind(), K::Document { document: None, .. }));
    }

    fn caption_of(text: &str) -> base::PageCaption {
        caption(text, "")
    }
}
