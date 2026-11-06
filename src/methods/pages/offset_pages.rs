/// Buffer, item limit and cursor of a pager. `C` is whatever Telegram takes to fetch the next page: a string token by
/// default, or a count or id for methods that page that way.
pub(crate) struct OffsetPages<T, C = String> {
    buffer: std::vec::IntoIter<T>,
    pub(crate) remaining: usize,
    page_size: usize,
    // `None` once the list has ended.
    offset: Option<C>,
}

impl<T, C> OffsetPages<T, C> {
    pub(crate) fn starting_at(page_size: usize, offset: C) -> Self {
        Self { buffer: Vec::new().into_iter(), remaining: usize::MAX, page_size, offset: Some(offset) }
    }

    pub(crate) fn pop(&mut self) -> Option<T> {
        if self.remaining == 0 {
            return None;
        }
        let item = self.buffer.next()?;
        self.remaining -= 1;
        Some(item)
    }

    /// How many items to ask for next, or `None` once the list or the limit has run out.
    pub(crate) fn limit(&self) -> Option<i32> {
        (self.offset.is_some() && self.remaining > 0).then(|| self.remaining.min(self.page_size) as i32)
    }

    /// Where the next page starts. `None` once the list has ended.
    pub(crate) fn offset(&self) -> Option<&C> {
        self.offset.as_ref()
    }

    /// Moves the start of the first page, for a caller continuing a list it read part of.
    pub(crate) fn set_offset(&mut self, offset: C) {
        self.offset = Some(offset);
    }

    /// Buffers a parsed page. `next` is where the page after it starts; `None` ends the list.
    pub(crate) fn push(&mut self, items: Vec<T>, next: Option<C>) {
        self.offset = next;
        self.buffer = items.into_iter();
    }
}

impl<T> OffsetPages<T> {
    pub(crate) fn new(page_size: usize) -> Self {
        Self::starting_at(page_size, String::new())
    }

    pub(crate) fn next_offset(&self) -> Option<&str> {
        self.offset.as_deref().filter(|o| !o.is_empty())
    }

    pub(crate) fn refill<R>(&mut self, raw: Vec<R>, next_offset: Option<String>, parse: impl FnOnce(Vec<R>) -> Vec<T>) {
        // A missing, empty or repeated offset ends the list, and so does an empty page: asking again would loop
        // forever.
        let repeated = |o: &String| self.offset.as_ref() == Some(o);
        let next = next_offset.filter(|o| !o.is_empty() && !raw.is_empty() && !repeated(o));
        self.push(parse(raw), next);
    }
}

impl<T> OffsetPages<T, i32> {
    /// Buffers a page of a list Telegram pages by count: the next page skips everything read so far. `read` is how
    /// many raw items the page held before parsing dropped any, and `total` how many the list holds in all.
    pub(crate) fn push_counted(&mut self, items: Vec<T>, read: usize, total: i32) {
        let skipped = self.offset.unwrap_or(0).saturating_add(super::wire_limit(read));
        // An empty page ends the list too: asking again would loop forever.
        self.push(items, (read > 0 && skipped < total).then_some(skipped));
    }
}

#[cfg(test)]
mod tests {
    use super::OffsetPages;

    #[test]
    fn empty_pages_and_offsets_end_the_list() {
        let keep_even = |raw: Vec<i32>| raw.into_iter().filter(|n| n % 2 == 0).collect();
        let mut pages = OffsetPages::new(50);
        pages.remaining = 120;
        assert_eq!((pages.limit(), pages.next_offset()), (Some(50), None));

        pages.refill(vec![1, 2, 4], Some("a".into()), keep_even);
        assert_eq!((pages.pop(), pages.pop(), pages.pop(), pages.remaining), (Some(2), Some(4), None, 118));
        assert_eq!((pages.limit(), pages.next_offset()), (Some(50), Some("a")));
        pages.refill(vec![1], Some("b".into()), keep_even);
        assert_eq!((pages.pop(), pages.next_offset()), (None, Some("b")));

        for (raw, next) in [(vec![], Some("c")), (vec![2], Some("")), (vec![2], None)] {
            let mut pages = OffsetPages::new(50);
            pages.refill(raw, next.map(Into::into), keep_even);
            assert_eq!(pages.limit(), None);
        }

        // A token that doesn't move would fetch the same page forever.
        let mut pages = OffsetPages::new(50);
        pages.set_offset("same".into());
        pages.refill(vec![2], Some("same".into()), keep_even);
        assert_eq!((pages.pop(), pages.limit()), (Some(2), None));
    }

    #[test]
    fn counted_pages_skip_what_was_read_and_stop_at_the_total() {
        let mut pages = OffsetPages::<u8, i32>::starting_at(2, 3);
        pages.push_counted(vec![1], 2, 10);
        assert_eq!((pages.pop(), pages.offset(), pages.limit()), (Some(1), Some(&5), Some(2)));
        pages.push_counted(vec![2, 3], 2, 7);
        assert_eq!(pages.limit(), None);

        let mut pages = OffsetPages::<u8, i32>::starting_at(2, 0);
        pages.push_counted(Vec::new(), 0, 10);
        assert_eq!(pages.limit(), None);
    }
}
