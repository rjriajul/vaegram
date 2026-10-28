use crate::raw::base;

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
