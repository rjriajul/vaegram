use std::sync::Arc;

use crate::errors::Result;
use crate::methods::geo_point;
use crate::raw::{base, types};
use crate::types::messages_and_media::{input_document, input_photo};
use crate::types::{Animation, Audio, Document, InlineKeyboardButton, Photo, RichText, Video};

/// Horizontal alignment of a button row or a table cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Align {
    /// Aligns to the left edge.
    Left,
    /// Centers horizontally.
    Center,
    /// Aligns to the right edge.
    Right,
}

/// Vertical alignment of a table cell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum VAlign {
    /// Aligns to the top of the cell.
    Top,
    /// Centers vertically.
    Middle,
    /// Aligns to the bottom of the cell.
    Bottom,
}

/// Color style of a [`RichButton`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum RichButtonStyle {
    /// No style is sent, so Telegram uses its regular button look.
    #[default]
    Default,
    /// Primary background color.
    Primary,
    /// Danger background color.
    Danger,
    /// Success background color.
    Success,
    /// Rendered as a link instead of a filled button.
    Link,
}

/// One block of a rich message, such as a paragraph, heading, list, table or photo.
///
/// Build blocks with the constructors ([`paragraph`](RichBlock::paragraph), [`list`](RichBlock::list) and so on),
/// then refine them with modifiers like [`caption`](RichBlock::caption). A modifier that doesn't apply to the
/// block's kind leaves it unchanged.
#[derive(Debug, Clone, PartialEq)]
pub struct RichBlock {
    pub(crate) block: base::PageBlock,
    pub(crate) files: Files,
}

/// One item of a [`RichBlock::list`] or [`RichBlock::ordered_list`].
#[derive(Debug, Clone, PartialEq)]
pub struct RichListItem {
    pub(super) checkbox: bool,
    pub(super) checked: bool,
    pub(super) blocks: Vec<RichBlock>,
    // Set when the item was read from a message: the label Telegram shows, and an ordered item's own number and
    // numbering type.
    pub(super) label: String,
    pub(super) value: Option<i32>,
    pub(super) numbering: Option<String>,
}

/// A cell of a [`RichBlock::table`] row.
#[derive(Debug, Clone, PartialEq)]
pub struct RichTableCell(pub(super) base::PageTableCell);

/// A button in a [`RichBlock::buttons`] row, made from an [`InlineKeyboardButton`].
#[derive(Debug, Clone, PartialEq)]
pub struct RichButton {
    pub(super) text: String,
    pub(super) kind: base::InlineButtonType,
    pub(super) style: RichButtonStyle,
}

/// The photos and documents of the message a block was read from, so a block can show its media.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Resolved {
    pub(crate) photos: Vec<base::Photo>,
    pub(crate) documents: Vec<base::Document>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Files {
    pub(crate) photos: Vec<base::InputPhoto>,
    pub(crate) documents: Vec<base::InputDocument>,
    pub(crate) source: Option<Arc<Resolved>>,
}

impl Files {
    fn add(&mut self, other: Files) {
        for photo in other.photos {
            self.add_photo(photo);
        }
        for document in other.documents {
            self.add_document(document);
        }
    }

    pub(crate) fn add_photo(&mut self, photo: base::InputPhoto) -> i64 {
        let id = match &photo {
            base::InputPhoto::InputPhoto(p) => p.id,
            base::InputPhoto::InputPhotoEmpty => 0,
        };
        if !self.photos.contains(&photo) {
            self.photos.push(photo);
        }
        id
    }

    pub(crate) fn add_document(&mut self, document: base::InputDocument) -> i64 {
        let id = match &document {
            base::InputDocument::InputDocument(d) => d.id,
            base::InputDocument::InputDocumentEmpty => 0,
        };
        if !self.documents.contains(&document) {
            self.documents.push(document);
        }
        id
    }
}

pub(crate) fn unzip(blocks: impl IntoIterator<Item = RichBlock>, files: &mut Files) -> Vec<base::PageBlock> {
    blocks
        .into_iter()
        .map(|b| {
            files.add(b.files);
            b.block
        })
        .collect()
}

fn text(text: impl Into<RichText>) -> base::RichText {
    text.into().0
}

impl RichBlock {
    fn new(block: base::PageBlock) -> Self {
        RichBlock { block, files: Files::default() }
    }

    /// Creates a paragraph of text.
    pub fn paragraph(text: impl Into<RichText>) -> Self {
        Self::new(base::PageBlock::PageBlockParagraph(Box::new(types::PageBlockParagraph { text: self::text(text) })))
    }

    /// Creates a heading. `level` is clamped to 1..=6.
    pub fn heading(level: u8, text: impl Into<RichText>) -> Self {
        use base::PageBlock as B;
        let text = self::text(text);
        Self::new(match level {
            0 | 1 => B::PageBlockHeading1(Box::new(types::PageBlockHeading1 { text })),
            2 => B::PageBlockHeading2(Box::new(types::PageBlockHeading2 { text })),
            3 => B::PageBlockHeading3(Box::new(types::PageBlockHeading3 { text })),
            4 => B::PageBlockHeading4(Box::new(types::PageBlockHeading4 { text })),
            5 => B::PageBlockHeading5(Box::new(types::PageBlockHeading5 { text })),
            _ => B::PageBlockHeading6(Box::new(types::PageBlockHeading6 { text })),
        })
    }

    /// Creates a preformatted code block tagged with `language`.
    pub fn preformatted(text: impl Into<RichText>, language: impl Into<String>) -> Self {
        let block = types::PageBlockPreformatted { text: self::text(text), language: language.into() };
        Self::new(base::PageBlock::PageBlockPreformatted(Box::new(block)))
    }

    /// Creates a footer line of text.
    pub fn footer(text: impl Into<RichText>) -> Self {
        Self::new(base::PageBlock::PageBlockFooter(Box::new(types::PageBlockFooter { text: self::text(text) })))
    }

    /// Creates a horizontal divider.
    pub fn divider() -> Self {
        Self::new(base::PageBlock::PageBlockDivider)
    }

    /// Creates a math block holding the formula `source`.
    pub fn math(source: impl Into<String>) -> Self {
        Self::new(base::PageBlock::PageBlockMath(Box::new(types::PageBlockMath { source: source.into() })))
    }

    /// Creates an anchor that links can point to by `name`.
    pub fn anchor(name: impl Into<String>) -> Self {
        Self::new(base::PageBlock::PageBlockAnchor(Box::new(types::PageBlockAnchor { name: name.into() })))
    }

    /// Creates a thinking block holding `text`.
    pub fn thinking(text: impl Into<RichText>) -> Self {
        Self::new(base::PageBlock::PageBlockThinking(Box::new(types::PageBlockThinking { text: self::text(text) })))
    }

    /// Creates a bulleted list. Media inside the items is carried along with the block.
    pub fn list(items: impl IntoIterator<Item = RichListItem>) -> Self {
        let mut files = Files::default();
        let items = items
            .into_iter()
            .map(|i| {
                let item = types::PageListItemBlocks {
                    checkbox: i.checkbox,
                    checked: i.checked,
                    blocks: unzip(i.blocks, &mut files),
                };
                base::PageListItem::PageListItemBlocks(Box::new(item))
            })
            .collect();
        RichBlock { block: base::PageBlock::PageBlockList(Box::new(types::PageBlockList { items })), files }
    }

    /// Creates a numbered list. Items are numbered from 1 in the order given.
    pub fn ordered_list(items: impl IntoIterator<Item = RichListItem>) -> Self {
        let mut files = Files::default();
        let items = items
            .into_iter()
            .zip(1..)
            .map(|(i, n): (_, u32)| {
                let (checkbox, checked, blocks) = (i.checkbox, i.checked, unzip(i.blocks, &mut files));
                let num = Some(n.to_string());
                let item =
                    types::PageListOrderedItemBlocks { checkbox, checked, num, blocks, value: None, r#type: None };
                base::PageListOrderedItem::PageListOrderedItemBlocks(Box::new(item))
            })
            .collect();
        let list = types::PageBlockOrderedList { reversed: false, items, start: None, r#type: None };
        RichBlock { block: base::PageBlock::PageBlockOrderedList(Box::new(list)), files }
    }

    /// Creates a quote that contains other blocks. Add an attribution with [`credit`](RichBlock::credit).
    pub fn blockquote(blocks: impl IntoIterator<Item = RichBlock>) -> Self {
        let mut files = Files::default();
        let blocks = unzip(blocks, &mut files);
        let quote = types::PageBlockBlockquoteBlocks { blocks, caption: base::RichText::TextEmpty };
        RichBlock { block: base::PageBlock::PageBlockBlockquoteBlocks(Box::new(quote)), files }
    }

    /// Creates a text quote that stays collapsed until the reader expands it.
    pub fn expandable_blockquote(text: impl Into<RichText>) -> Self {
        let quote =
            types::PageBlockBlockquote { collapsed: true, text: self::text(text), caption: base::RichText::TextEmpty };
        Self::new(base::PageBlock::PageBlockBlockquote(Box::new(quote)))
    }

    /// Creates a pullquote, a highlighted excerpt. Add an attribution with [`credit`](RichBlock::credit).
    pub fn pullquote(text: impl Into<RichText>) -> Self {
        let quote = types::PageBlockPullquote { text: self::text(text), caption: base::RichText::TextEmpty };
        Self::new(base::PageBlock::PageBlockPullquote(Box::new(quote)))
    }

    /// Creates a collage of media blocks such as photos and videos.
    pub fn collage(items: impl IntoIterator<Item = RichBlock>) -> Self {
        let mut files = Files::default();
        let collage = types::PageBlockCollage { items: unzip(items, &mut files), caption: caption() };
        RichBlock { block: base::PageBlock::PageBlockCollage(Box::new(collage)), files }
    }

    /// Creates a slideshow of media blocks such as photos and videos.
    pub fn slideshow(items: impl IntoIterator<Item = RichBlock>) -> Self {
        let mut files = Files::default();
        let slideshow = types::PageBlockSlideshow { items: unzip(items, &mut files), caption: caption() };
        RichBlock { block: base::PageBlock::PageBlockSlideshow(Box::new(slideshow)), files }
    }

    /// Creates a table from rows of cells. It starts without borders, stripes or a title.
    ///
    /// See [`bordered`](RichBlock::bordered), [`striped`](RichBlock::striped), [`compact`](RichBlock::compact) and
    /// [`caption`](RichBlock::caption).
    pub fn table(rows: impl IntoIterator<Item = Vec<RichTableCell>>) -> Self {
        let rows = rows
            .into_iter()
            .map(|cells| {
                let cells = cells.into_iter().map(|c| c.0).collect();
                base::PageTableRow::PageTableRow(Box::new(types::PageTableRow { cells }))
            })
            .collect();
        let table = types::PageBlockTable {
            bordered: false,
            striped: false,
            compact: false,
            title: base::RichText::TextEmpty,
            rows,
        };
        Self::new(base::PageBlock::PageBlockTable(Box::new(table)))
    }

    /// Creates a collapsible section with a `summary` line and hidden `blocks`. It starts collapsed unless
    /// [`open`](RichBlock::open) is called.
    pub fn details(summary: impl Into<RichText>, blocks: impl IntoIterator<Item = RichBlock>) -> Self {
        let mut files = Files::default();
        let details = types::PageBlockDetails { open: false, blocks: unzip(blocks, &mut files), title: text(summary) };
        RichBlock { block: base::PageBlock::PageBlockDetails(Box::new(details)), files }
    }

    /// Creates a map centered on a point, at `zoom` in a `width` by `height` frame.
    pub fn map(latitude: f64, longitude: f64, zoom: i32, width: i32, height: i32) -> Self {
        let map = types::InputPageBlockMap {
            geo: geo_point(latitude, longitude, None),
            zoom,
            w: width,
            h: height,
            caption: caption(),
        };
        Self::new(base::PageBlock::InputPageBlockMap(Box::new(map)))
    }

    /// Creates a row of buttons. No alignment is sent unless [`align`](RichBlock::align) sets one.
    pub fn buttons(row: impl IntoIterator<Item = RichButton>) -> Self {
        let buttons = row.into_iter().map(RichButton::write).collect();
        let row = types::PageBlockButtonRow { align_left: false, align_center: false, align_right: false, buttons };
        Self::new(base::PageBlock::PageBlockButtonRow(Box::new(row)))
    }

    /// Shows an existing [`Photo`] by reference, without uploading it again.
    pub fn photo(photo: &Photo) -> Self {
        let mut files = Files::default();
        let photo_id = files.add_photo(input_photo(&photo.location));
        let block = types::PageBlockPhoto { spoiler: false, photo_id, caption: caption(), url: None, webpage_id: None };
        RichBlock { block: base::PageBlock::PageBlockPhoto(Box::new(block)), files }
    }

    /// Shows an existing [`Video`] by reference, without uploading it again.
    pub fn video(video: &Video) -> Self {
        Self::video_block(input_document(&video.location), false)
    }

    /// Shows an existing [`Animation`] as a video block that autoplays and loops.
    pub fn animation(animation: &Animation) -> Self {
        Self::video_block(input_document(&animation.location), true)
    }

    fn video_block(video: base::InputDocument, animated: bool) -> Self {
        let mut files = Files::default();
        let video_id = files.add_document(video);
        let block = types::PageBlockVideo {
            autoplay: animated,
            r#loop: animated,
            spoiler: false,
            video_id,
            caption: caption(),
        };
        RichBlock { block: base::PageBlock::PageBlockVideo(Box::new(block)), files }
    }

    /// Shows an existing [`Audio`] by reference, without uploading it again.
    pub fn audio(audio: &Audio) -> Self {
        let mut files = Files::default();
        let audio_id = files.add_document(input_document(&audio.location));
        let block = base::PageBlock::PageBlockAudio(Box::new(types::PageBlockAudio { audio_id, caption: caption() }));
        RichBlock { block, files }
    }

    /// Shows an existing [`Document`] by reference, without uploading it again.
    pub fn document(document: &Document) -> Self {
        let mut files = Files::default();
        let document_id = files.add_document(input_document(&document.location));
        let block = types::PageBlockDocument { document_id, caption: caption() };
        RichBlock { block: base::PageBlock::PageBlockDocument(Box::new(block)), files }
    }

    /// Titles a table, or captions a photo, video, audio, document, collage, slideshow or map. Other blocks
    /// ignore it.
    #[must_use]
    pub fn caption(mut self, text: impl Into<RichText>) -> Self {
        if let base::PageBlock::PageBlockTable(b) = &mut self.block {
            b.title = self::text(text);
        } else if let Some(base::PageCaption::PageCaption(c)) = page_caption(&mut self.block) {
            c.text = self::text(text);
        }
        self
    }

    /// Credits a quote, pullquote or a block that takes a caption. Other blocks ignore it.
    #[must_use]
    pub fn credit(mut self, text: impl Into<RichText>) -> Self {
        use base::PageBlock as B;
        let text = self::text(text);
        match &mut self.block {
            B::PageBlockBlockquote(b) => b.caption = text,
            B::PageBlockBlockquoteBlocks(b) => b.caption = text,
            B::PageBlockPullquote(b) => b.caption = text,
            block => {
                if let Some(base::PageCaption::PageCaption(c)) = page_caption(block) {
                    c.credit = text;
                }
            }
        }
        self
    }

    /// Hides a photo or video behind a spoiler. Other blocks ignore it.
    #[must_use]
    pub fn spoiler(mut self) -> Self {
        match &mut self.block {
            base::PageBlock::PageBlockPhoto(b) => b.spoiler = true,
            base::PageBlock::PageBlockVideo(b) => b.spoiler = true,
            _ => {}
        }
        self
    }

    /// Shows a details block expanded. Other blocks ignore it.
    #[must_use]
    pub fn open(mut self) -> Self {
        if let base::PageBlock::PageBlockDetails(b) = &mut self.block {
            b.open = true;
        }
        self
    }

    /// Draws borders around table cells. Other blocks ignore it.
    #[must_use]
    pub fn bordered(mut self) -> Self {
        if let base::PageBlock::PageBlockTable(b) = &mut self.block {
            b.bordered = true;
        }
        self
    }

    /// Shades alternate table rows. Other blocks ignore it.
    #[must_use]
    pub fn striped(mut self) -> Self {
        if let base::PageBlock::PageBlockTable(b) = &mut self.block {
            b.striped = true;
        }
        self
    }

    /// Tightens table cell spacing. Other blocks ignore it.
    #[must_use]
    pub fn compact(mut self) -> Self {
        if let base::PageBlock::PageBlockTable(b) = &mut self.block {
            b.compact = true;
        }
        self
    }

    /// Aligns the buttons of a row. Other blocks ignore it.
    #[must_use]
    pub fn align(mut self, align: Align) -> Self {
        if let base::PageBlock::PageBlockButtonRow(b) = &mut self.block {
            (b.align_left, b.align_center, b.align_right) =
                (align == Align::Left, align == Align::Center, align == Align::Right);
        }
        self
    }
}

impl RichListItem {
    /// Creates an item holding one paragraph of `text`.
    pub fn new(text: impl Into<RichText>) -> Self {
        Self::blocks([RichBlock::paragraph(text)])
    }

    /// Creates an item made of several blocks instead of a single paragraph.
    pub fn blocks(blocks: impl IntoIterator<Item = RichBlock>) -> Self {
        RichListItem {
            checkbox: false,
            checked: false,
            blocks: blocks.into_iter().collect(),
            label: String::new(),
            value: None,
            numbering: None,
        }
    }

    /// Creates an item with a checkbox, ticked when `checked` is true.
    pub fn checkbox(text: impl Into<RichText>, checked: bool) -> Self {
        RichListItem { checkbox: true, checked, ..Self::new(text) }
    }
}

impl RichTableCell {
    /// Creates a body cell holding `text`, with no alignment or span set.
    pub fn new(text: impl Into<RichText>) -> Self {
        RichTableCell(base::PageTableCell::PageTableCell(Box::new(types::PageTableCell {
            header: false,
            align_center: false,
            align_right: false,
            valign_middle: false,
            valign_bottom: false,
            text: Some(self::text(text)),
            colspan: None,
            rowspan: None,
        })))
    }

    /// Marks the cell as a header cell.
    #[must_use]
    pub fn header(mut self) -> Self {
        let base::PageTableCell::PageTableCell(c) = &mut self.0;
        c.header = true;
        self
    }

    /// Aligns the cell text horizontally.
    #[must_use]
    pub fn align(mut self, align: Align) -> Self {
        let base::PageTableCell::PageTableCell(c) = &mut self.0;
        (c.align_center, c.align_right) = (align == Align::Center, align == Align::Right);
        self
    }

    /// Aligns the cell text vertically.
    #[must_use]
    pub fn valign(mut self, valign: VAlign) -> Self {
        let base::PageTableCell::PageTableCell(c) = &mut self.0;
        (c.valign_middle, c.valign_bottom) = (valign == VAlign::Middle, valign == VAlign::Bottom);
        self
    }

    /// Makes the cell span `columns` columns.
    #[must_use]
    pub fn colspan(mut self, columns: i32) -> Self {
        let base::PageTableCell::PageTableCell(c) = &mut self.0;
        c.colspan = Some(columns);
        self
    }

    /// Makes the cell span `rows` rows.
    #[must_use]
    pub fn rowspan(mut self, rows: i32) -> Self {
        let base::PageTableCell::PageTableCell(c) = &mut self.0;
        c.rowspan = Some(rows);
        self
    }
}

impl RichButton {
    /// Converts an inline keyboard button into a rich message button.
    ///
    /// Fails for a button that names a user (a profile button, or a login button naming a bot), because it
    /// can't be written without looking the user up.
    pub fn new(button: &InlineKeyboardButton) -> Result<Self> {
        let base::KeyboardInlineButton::KeyboardInlineButton(b) = button.write_offline()?;
        Ok(RichButton { text: b.text, kind: b.r#type, style: RichButtonStyle::Default })
    }

    /// Changes the button's color style. [`RichButtonStyle::Default`] sends no style.
    #[must_use]
    pub fn style(mut self, style: RichButtonStyle) -> Self {
        self.style = style;
        self
    }

    fn write(self) -> base::PageButton {
        let style = self.style.write();
        base::PageButton::PageButton(Box::new(types::PageButton { text: text(self.text), r#type: self.kind, style }))
    }
}

pub(crate) fn caption() -> base::PageCaption {
    let caption = types::PageCaption { text: base::RichText::TextEmpty, credit: base::RichText::TextEmpty };
    base::PageCaption::PageCaption(Box::new(caption))
}

impl RichButtonStyle {
    fn write(self) -> Option<base::RichButtonStyle> {
        let (bg_primary, bg_danger, bg_success, link) = match self {
            RichButtonStyle::Default => return None,
            RichButtonStyle::Primary => (true, false, false, false),
            RichButtonStyle::Danger => (false, true, false, false),
            RichButtonStyle::Success => (false, false, true, false),
            RichButtonStyle::Link => (false, false, false, true),
        };
        let style = types::RichButtonStyle { bg_primary, bg_danger, bg_success, link };
        Some(base::RichButtonStyle::RichButtonStyle(style))
    }
}

fn page_caption(block: &mut base::PageBlock) -> Option<&mut base::PageCaption> {
    use base::PageBlock as B;
    Some(match block {
        B::PageBlockPhoto(b) => &mut b.caption,
        B::PageBlockVideo(b) => &mut b.caption,
        B::PageBlockAudio(b) => &mut b.caption,
        B::PageBlockDocument(b) => &mut b.caption,
        B::PageBlockCollage(b) => &mut b.caption,
        B::PageBlockSlideshow(b) => &mut b.caption,
        B::PageBlockMap(b) => &mut b.caption,
        B::InputPageBlockMap(b) => &mut b.caption,
        _ => return None,
    })
}

// One walk over every rich text of a block, written once for shared and once for mutable references.
macro_rules! text_walk {
    ($(#[$doc:meta])* $name:ident $(, $m:tt)?) => {
        $(#[$doc])*
        pub(crate) fn $name(block: &$($m)? base::PageBlock, f: &mut impl FnMut(&$($m)? base::RichText)) {
            use base::PageBlock as B;
            fn caption(caption: &$($m)? base::PageCaption, f: &mut impl FnMut(&$($m)? base::RichText)) {
                let base::PageCaption::PageCaption(c) = caption;
                f(&$($m)? c.text);
                f(&$($m)? c.credit);
            }
            fn blocks(blocks: &$($m)? [base::PageBlock], f: &mut impl FnMut(&$($m)? base::RichText)) {
                for b in blocks {
                    $name(b, f);
                }
            }
            match block {
                B::PageBlockUnsupported
                | B::PageBlockDivider
                | B::PageBlockAnchor(_)
                | B::PageBlockChannel(_)
                | B::PageBlockMath(_) => {}
                B::PageBlockTitle(b) => f(&$($m)? b.text),
                B::PageBlockSubtitle(b) => f(&$($m)? b.text),
                B::PageBlockAuthorDate(b) => f(&$($m)? b.author),
                B::PageBlockHeader(b) => f(&$($m)? b.text),
                B::PageBlockSubheader(b) => f(&$($m)? b.text),
                B::PageBlockParagraph(b) => f(&$($m)? b.text),
                B::PageBlockPreformatted(b) => f(&$($m)? b.text),
                B::PageBlockFooter(b) => f(&$($m)? b.text),
                B::PageBlockKicker(b) => f(&$($m)? b.text),
                B::PageBlockHeading1(b) => f(&$($m)? b.text),
                B::PageBlockHeading2(b) => f(&$($m)? b.text),
                B::PageBlockHeading3(b) => f(&$($m)? b.text),
                B::PageBlockHeading4(b) => f(&$($m)? b.text),
                B::PageBlockHeading5(b) => f(&$($m)? b.text),
                B::PageBlockHeading6(b) => f(&$($m)? b.text),
                B::PageBlockThinking(b) => f(&$($m)? b.text),
                B::PageBlockBlockquote(b) => {
                    f(&$($m)? b.text);
                    f(&$($m)? b.caption);
                }
                B::PageBlockPullquote(b) => {
                    f(&$($m)? b.text);
                    f(&$($m)? b.caption);
                }
                B::PageBlockBlockquoteBlocks(b) => {
                    blocks(&$($m)? b.blocks, f);
                    f(&$($m)? b.caption);
                }
                B::PageBlockList(b) => {
                    for item in &$($m)? b.items {
                        match item {
                            base::PageListItem::PageListItemText(i) => f(&$($m)? i.text),
                            base::PageListItem::PageListItemBlocks(i) => blocks(&$($m)? i.blocks, f),
                        }
                    }
                }
                B::PageBlockOrderedList(b) => {
                    for item in &$($m)? b.items {
                        match item {
                            base::PageListOrderedItem::PageListOrderedItemText(i) => f(&$($m)? i.text),
                            base::PageListOrderedItem::PageListOrderedItemBlocks(i) => blocks(&$($m)? i.blocks, f),
                        }
                    }
                }
                B::PageBlockTable(b) => {
                    f(&$($m)? b.title);
                    for base::PageTableRow::PageTableRow(row) in &$($m)? b.rows {
                        for base::PageTableCell::PageTableCell(c) in &$($m)? row.cells {
                            if let Some(text) = &$($m)? c.text {
                                f(text);
                            }
                        }
                    }
                }
                B::PageBlockDetails(b) => {
                    f(&$($m)? b.title);
                    blocks(&$($m)? b.blocks, f);
                }
                B::PageBlockRelatedArticles(b) => f(&$($m)? b.title),
                B::PageBlockCover(b) => $name(&$($m)? b.cover, f),
                B::PageBlockCollage(b) => {
                    blocks(&$($m)? b.items, f);
                    caption(&$($m)? b.caption, f);
                }
                B::PageBlockSlideshow(b) => {
                    blocks(&$($m)? b.items, f);
                    caption(&$($m)? b.caption, f);
                }
                B::PageBlockEmbedPost(b) => {
                    blocks(&$($m)? b.blocks, f);
                    caption(&$($m)? b.caption, f);
                }
                B::PageBlockPhoto(b) => caption(&$($m)? b.caption, f),
                B::PageBlockVideo(b) => caption(&$($m)? b.caption, f),
                B::PageBlockAudio(b) => caption(&$($m)? b.caption, f),
                B::PageBlockDocument(b) => caption(&$($m)? b.caption, f),
                B::PageBlockEmbed(b) => caption(&$($m)? b.caption, f),
                B::PageBlockMap(b) => caption(&$($m)? b.caption, f),
                B::InputPageBlockMap(b) => caption(&$($m)? b.caption, f),
                B::PageBlockButtonRow(b) => {
                    for base::PageButton::PageButton(button) in &$($m)? b.buttons {
                        f(&$($m)? button.text);
                    }
                }
            }
        }
    };
}

text_walk!(
    /// Calls `f` with every rich text of `block`, nested blocks included.
    for_each_text
);
// rustfmt 1.100 splits `for_each_text_mut, mut` over two lines and older releases join it again.
#[rustfmt::skip]
text_walk!(
    /// [`for_each_text`], with each text open to change.
    for_each_text_mut, mut
);

#[cfg(test)]
mod tests {
    use super::{Align, RichBlock, RichButton, RichButtonStyle, RichListItem, RichTableCell, VAlign};
    use crate::raw::{base, types};
    use crate::types::{InlineKeyboardButton, RichText, Text};

    #[test]
    fn blocks_map_to_raw() {
        let list = RichBlock::ordered_list([RichListItem::checkbox("done", true)]);
        let base::PageBlock::PageBlockOrderedList(list) = list.block else { panic!("expected an ordered list") };
        let base::PageListOrderedItem::PageListOrderedItemBlocks(item) = &list.items[0] else {
            panic!("expected block items");
        };
        assert_eq!((item.checkbox, item.checked, item.num.as_deref()), (true, true, Some("1")));
        assert_eq!(item.blocks, vec![RichBlock::paragraph("done").block]);
        let base::PageBlock::PageBlockPullquote(q) = RichBlock::pullquote("q").credit("me").block else {
            panic!("expected a quote")
        };
        assert_eq!(q.caption, RichText::from("me").0);

        let RichTableCell(base::PageTableCell::PageTableCell(c)) =
            RichTableCell::new("x").header().align(Align::Right).valign(VAlign::Middle);
        assert_eq!((c.header, c.align_center, c.align_right, c.valign_middle), (true, false, true, true));

        let button =
            RichButton::new(&InlineKeyboardButton::url("Docs", "https://t.me")).unwrap().style(RichButtonStyle::Link);
        let base::PageBlock::PageBlockButtonRow(row) = RichBlock::buttons([button]).align(Align::Center).block else {
            panic!("expected buttons")
        };
        let base::PageButton::PageButton(b) = &row.buttons[0];
        let url = types::InlineButtonTypeUrl { url: "https://t.me".into() };
        assert_eq!(b.r#type, base::InlineButtonType::InlineButtonTypeUrl(Box::new(url)));
        let style = types::RichButtonStyle { bg_primary: false, bg_danger: false, bg_success: false, link: true };
        assert_eq!(b.style, Some(base::RichButtonStyle::RichButtonStyle(style)));
        assert_eq!((row.align_left, row.align_center, b.text.clone()), (false, true, RichText::from("Docs").0));

        let details = RichBlock::details(Text::new().italic("more"), [RichBlock::divider()]).open();
        let base::PageBlock::PageBlockDetails(d) = details.block else { panic!("expected details") };
        assert!(d.open);
        assert!(matches!(RichBlock::heading(9, "h").block, base::PageBlock::PageBlockHeading6(_)));
    }
}
