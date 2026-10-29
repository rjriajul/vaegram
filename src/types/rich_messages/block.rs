use crate::methods::geo_point;
use crate::raw::{base, types};
use crate::types::messages_and_media::{input_document, input_photo};
use crate::types::{Animation, Audio, Document, InlineKeyboardButton, Photo, RichText, Video};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VAlign {
    Top,
    Middle,
    Bottom,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RichButtonStyle {
    #[default]
    Default,
    Primary,
    Danger,
    Success,
    Link,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RichBlock {
    pub(crate) block: base::PageBlock,
    pub(crate) files: Files,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RichListItem {
    checkbox: bool,
    checked: bool,
    blocks: Vec<RichBlock>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RichTableCell(base::PageTableCell);

#[derive(Debug, Clone, PartialEq)]
pub struct RichButton {
    text: String,
    kind: base::InlineButtonType,
    style: RichButtonStyle,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Files {
    pub(crate) photos: Vec<base::InputPhoto>,
    pub(crate) documents: Vec<base::InputDocument>,
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

    pub fn paragraph(text: impl Into<RichText>) -> Self {
        Self::new(base::PageBlock::PageBlockParagraph(Box::new(types::PageBlockParagraph { text: self::text(text) })))
    }

    /// `level` is clamped to 1..=6.
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

    pub fn preformatted(text: impl Into<RichText>, language: impl Into<String>) -> Self {
        let block = types::PageBlockPreformatted { text: self::text(text), language: language.into() };
        Self::new(base::PageBlock::PageBlockPreformatted(Box::new(block)))
    }

    pub fn footer(text: impl Into<RichText>) -> Self {
        Self::new(base::PageBlock::PageBlockFooter(Box::new(types::PageBlockFooter { text: self::text(text) })))
    }

    pub fn divider() -> Self {
        Self::new(base::PageBlock::PageBlockDivider)
    }

    pub fn math(source: impl Into<String>) -> Self {
        Self::new(base::PageBlock::PageBlockMath(Box::new(types::PageBlockMath { source: source.into() })))
    }

    pub fn anchor(name: impl Into<String>) -> Self {
        Self::new(base::PageBlock::PageBlockAnchor(Box::new(types::PageBlockAnchor { name: name.into() })))
    }

    pub fn thinking(text: impl Into<RichText>) -> Self {
        Self::new(base::PageBlock::PageBlockThinking(Box::new(types::PageBlockThinking { text: self::text(text) })))
    }

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

    pub fn blockquote(blocks: impl IntoIterator<Item = RichBlock>) -> Self {
        let mut files = Files::default();
        let blocks = unzip(blocks, &mut files);
        let quote = types::PageBlockBlockquoteBlocks { blocks, caption: base::RichText::TextEmpty };
        RichBlock { block: base::PageBlock::PageBlockBlockquoteBlocks(Box::new(quote)), files }
    }

    pub fn expandable_blockquote(text: impl Into<RichText>) -> Self {
        let quote =
            types::PageBlockBlockquote { collapsed: true, text: self::text(text), caption: base::RichText::TextEmpty };
        Self::new(base::PageBlock::PageBlockBlockquote(Box::new(quote)))
    }

    pub fn pullquote(text: impl Into<RichText>) -> Self {
        let quote = types::PageBlockPullquote { text: self::text(text), caption: base::RichText::TextEmpty };
        Self::new(base::PageBlock::PageBlockPullquote(Box::new(quote)))
    }

    pub fn collage(items: impl IntoIterator<Item = RichBlock>) -> Self {
        let mut files = Files::default();
        let collage = types::PageBlockCollage { items: unzip(items, &mut files), caption: caption() };
        RichBlock { block: base::PageBlock::PageBlockCollage(Box::new(collage)), files }
    }

    pub fn slideshow(items: impl IntoIterator<Item = RichBlock>) -> Self {
        let mut files = Files::default();
        let slideshow = types::PageBlockSlideshow { items: unzip(items, &mut files), caption: caption() };
        RichBlock { block: base::PageBlock::PageBlockSlideshow(Box::new(slideshow)), files }
    }

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

    pub fn details(summary: impl Into<RichText>, blocks: impl IntoIterator<Item = RichBlock>) -> Self {
        let mut files = Files::default();
        let details = types::PageBlockDetails { open: false, blocks: unzip(blocks, &mut files), title: text(summary) };
        RichBlock { block: base::PageBlock::PageBlockDetails(Box::new(details)), files }
    }

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

    pub fn buttons(row: impl IntoIterator<Item = RichButton>) -> Self {
        let buttons = row.into_iter().map(RichButton::write).collect();
        let row = types::PageBlockButtonRow { align_left: false, align_center: false, align_right: false, buttons };
        Self::new(base::PageBlock::PageBlockButtonRow(Box::new(row)))
    }

    pub fn photo(photo: &Photo) -> Self {
        let mut files = Files::default();
        let photo_id = files.add_photo(input_photo(&photo.location));
        let block = types::PageBlockPhoto { spoiler: false, photo_id, caption: caption(), url: None, webpage_id: None };
        RichBlock { block: base::PageBlock::PageBlockPhoto(Box::new(block)), files }
    }

    pub fn video(video: &Video) -> Self {
        Self::video_block(input_document(&video.location), false)
    }

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

    pub fn audio(audio: &Audio) -> Self {
        let mut files = Files::default();
        let audio_id = files.add_document(input_document(&audio.location));
        let block = base::PageBlock::PageBlockAudio(Box::new(types::PageBlockAudio { audio_id, caption: caption() }));
        RichBlock { block, files }
    }

    pub fn document(document: &Document) -> Self {
        let mut files = Files::default();
        let document_id = files.add_document(input_document(&document.location));
        let block = types::PageBlockDocument { document_id, caption: caption() };
        RichBlock { block: base::PageBlock::PageBlockDocument(Box::new(block)), files }
    }

    /// Titles a table; captions a photo, video, audio, document, collage, slideshow or map; other blocks ignore it.
    pub fn caption(mut self, text: impl Into<RichText>) -> Self {
        if let base::PageBlock::PageBlockTable(b) = &mut self.block {
            b.title = self::text(text);
        } else if let Some(base::PageCaption::PageCaption(c)) = page_caption(&mut self.block) {
            c.text = self::text(text);
        }
        self
    }

    /// Credits a quote, pullquote or a block that takes a caption; other blocks ignore it.
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

    /// Hides a photo or video behind a spoiler; other blocks ignore it.
    pub fn spoiler(mut self) -> Self {
        match &mut self.block {
            base::PageBlock::PageBlockPhoto(b) => b.spoiler = true,
            base::PageBlock::PageBlockVideo(b) => b.spoiler = true,
            _ => {}
        }
        self
    }

    /// Shows a details block expanded; other blocks ignore it.
    pub fn open(mut self) -> Self {
        if let base::PageBlock::PageBlockDetails(b) = &mut self.block {
            b.open = true;
        }
        self
    }

    /// Tables only.
    pub fn bordered(mut self) -> Self {
        if let base::PageBlock::PageBlockTable(b) = &mut self.block {
            b.bordered = true;
        }
        self
    }

    /// Tables only.
    pub fn striped(mut self) -> Self {
        if let base::PageBlock::PageBlockTable(b) = &mut self.block {
            b.striped = true;
        }
        self
    }

    /// Tables only.
    pub fn compact(mut self) -> Self {
        if let base::PageBlock::PageBlockTable(b) = &mut self.block {
            b.compact = true;
        }
        self
    }

    /// Button rows only.
    pub fn align(mut self, align: Align) -> Self {
        if let base::PageBlock::PageBlockButtonRow(b) = &mut self.block {
            (b.align_left, b.align_center, b.align_right) =
                (align == Align::Left, align == Align::Center, align == Align::Right);
        }
        self
    }
}

impl RichListItem {
    pub fn new(text: impl Into<RichText>) -> Self {
        Self::blocks([RichBlock::paragraph(text)])
    }

    pub fn blocks(blocks: impl IntoIterator<Item = RichBlock>) -> Self {
        RichListItem { checkbox: false, checked: false, blocks: blocks.into_iter().collect() }
    }

    pub fn checkbox(text: impl Into<RichText>, checked: bool) -> Self {
        RichListItem { checkbox: true, checked, ..Self::new(text) }
    }
}

impl RichTableCell {
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

    pub fn header(mut self) -> Self {
        let base::PageTableCell::PageTableCell(c) = &mut self.0;
        c.header = true;
        self
    }

    pub fn align(mut self, align: Align) -> Self {
        let base::PageTableCell::PageTableCell(c) = &mut self.0;
        (c.align_center, c.align_right) = (align == Align::Center, align == Align::Right);
        self
    }

    pub fn valign(mut self, valign: VAlign) -> Self {
        let base::PageTableCell::PageTableCell(c) = &mut self.0;
        (c.valign_middle, c.valign_bottom) = (valign == VAlign::Middle, valign == VAlign::Bottom);
        self
    }

    pub fn colspan(mut self, columns: i32) -> Self {
        let base::PageTableCell::PageTableCell(c) = &mut self.0;
        c.colspan = Some(columns);
        self
    }

    pub fn rowspan(mut self, rows: i32) -> Self {
        let base::PageTableCell::PageTableCell(c) = &mut self.0;
        c.rowspan = Some(rows);
        self
    }
}

impl RichButton {
    pub fn new(button: &InlineKeyboardButton) -> Self {
        let base::KeyboardInlineButton::KeyboardInlineButton(b) = button.write();
        RichButton { text: b.text, kind: b.r#type, style: RichButtonStyle::Default }
    }

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

pub(crate) fn for_each_text<'a>(block: &'a base::PageBlock, f: &mut impl FnMut(&'a base::RichText)) {
    use base::PageBlock as B;
    fn caption<'a>(caption: &'a base::PageCaption, f: &mut impl FnMut(&'a base::RichText)) {
        let base::PageCaption::PageCaption(c) = caption;
        f(&c.text);
        f(&c.credit);
    }
    fn blocks<'a>(blocks: &'a [base::PageBlock], f: &mut impl FnMut(&'a base::RichText)) {
        blocks.iter().for_each(|b| for_each_text(b, f));
    }
    match block {
        B::PageBlockUnsupported
        | B::PageBlockDivider
        | B::PageBlockAnchor(_)
        | B::PageBlockChannel(_)
        | B::PageBlockMath(_) => {}
        B::PageBlockTitle(b) => f(&b.text),
        B::PageBlockSubtitle(b) => f(&b.text),
        B::PageBlockAuthorDate(b) => f(&b.author),
        B::PageBlockHeader(b) => f(&b.text),
        B::PageBlockSubheader(b) => f(&b.text),
        B::PageBlockParagraph(b) => f(&b.text),
        B::PageBlockPreformatted(b) => f(&b.text),
        B::PageBlockFooter(b) => f(&b.text),
        B::PageBlockKicker(b) => f(&b.text),
        B::PageBlockHeading1(b) => f(&b.text),
        B::PageBlockHeading2(b) => f(&b.text),
        B::PageBlockHeading3(b) => f(&b.text),
        B::PageBlockHeading4(b) => f(&b.text),
        B::PageBlockHeading5(b) => f(&b.text),
        B::PageBlockHeading6(b) => f(&b.text),
        B::PageBlockThinking(b) => f(&b.text),
        B::PageBlockBlockquote(b) => {
            f(&b.text);
            f(&b.caption);
        }
        B::PageBlockPullquote(b) => {
            f(&b.text);
            f(&b.caption);
        }
        B::PageBlockBlockquoteBlocks(b) => {
            blocks(&b.blocks, f);
            f(&b.caption);
        }
        B::PageBlockList(b) => b.items.iter().for_each(|item| match item {
            base::PageListItem::PageListItemText(i) => f(&i.text),
            base::PageListItem::PageListItemBlocks(i) => blocks(&i.blocks, f),
        }),
        B::PageBlockOrderedList(b) => b.items.iter().for_each(|item| match item {
            base::PageListOrderedItem::PageListOrderedItemText(i) => f(&i.text),
            base::PageListOrderedItem::PageListOrderedItemBlocks(i) => blocks(&i.blocks, f),
        }),
        B::PageBlockTable(b) => {
            f(&b.title);
            for base::PageTableRow::PageTableRow(row) in &b.rows {
                for base::PageTableCell::PageTableCell(c) in &row.cells {
                    if let Some(text) = &c.text {
                        f(text);
                    }
                }
            }
        }
        B::PageBlockDetails(b) => {
            f(&b.title);
            blocks(&b.blocks, f);
        }
        B::PageBlockRelatedArticles(b) => f(&b.title),
        B::PageBlockCover(b) => for_each_text(&b.cover, f),
        B::PageBlockCollage(b) => {
            blocks(&b.items, f);
            caption(&b.caption, f);
        }
        B::PageBlockSlideshow(b) => {
            blocks(&b.items, f);
            caption(&b.caption, f);
        }
        B::PageBlockEmbedPost(b) => {
            blocks(&b.blocks, f);
            caption(&b.caption, f);
        }
        B::PageBlockPhoto(b) => caption(&b.caption, f),
        B::PageBlockVideo(b) => caption(&b.caption, f),
        B::PageBlockAudio(b) => caption(&b.caption, f),
        B::PageBlockDocument(b) => caption(&b.caption, f),
        B::PageBlockEmbed(b) => caption(&b.caption, f),
        B::PageBlockMap(b) => caption(&b.caption, f),
        B::InputPageBlockMap(b) => caption(&b.caption, f),
        B::PageBlockButtonRow(b) => {
            for base::PageButton::PageButton(button) in &b.buttons {
                f(&button.text);
            }
        }
    }
}

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

        let button = RichButton::new(&InlineKeyboardButton::url("Docs", "https://t.me")).style(RichButtonStyle::Link);
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
