pub(crate) struct OffsetPages<T> {
    buffer: std::vec::IntoIter<T>,
    pub(crate) remaining: usize,
    page_size: usize,
    offset: Option<String>,
}

impl<T> OffsetPages<T> {
    pub(crate) fn new(page_size: usize) -> Self {
        Self { buffer: Vec::new().into_iter(), remaining: usize::MAX, page_size, offset: Some(String::new()) }
    }

    pub(crate) fn pop(&mut self) -> Option<T> {
        if self.remaining == 0 {
            return None;
        }
        let item = self.buffer.next()?;
        self.remaining -= 1;
        Some(item)
    }

    pub(crate) fn limit(&self) -> Option<i32> {
        (self.offset.is_some() && self.remaining > 0).then(|| self.remaining.min(self.page_size) as i32)
    }

    pub(crate) fn next_offset(&self) -> Option<&str> {
        self.offset.as_deref().filter(|o| !o.is_empty())
    }

    pub(crate) fn refill<R>(&mut self, raw: Vec<R>, next_offset: Option<String>, parse: impl FnOnce(Vec<R>) -> Vec<T>) {
        // A missing or empty offset ends the list, and so does an empty page: asking again would loop forever.
        self.offset = next_offset.filter(|o| !o.is_empty() && !raw.is_empty());
        self.buffer = parse(raw).into_iter();
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
    }
}
