use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::base;

const PAGE_SIZE: usize = 100;

pub(crate) struct DialogPages<T> {
    buffer: std::vec::IntoIter<T>,
    pub(crate) remaining: usize,
    pub(crate) offset_date: i32,
    pub(crate) offset_id: i32,
    pub(crate) offset_peer: base::InputPeer,
    done: bool,
    widened: bool,
}

impl<T> DialogPages<T> {
    pub(crate) fn new() -> Self {
        Self {
            buffer: Vec::new().into_iter(),
            remaining: usize::MAX,
            offset_date: 0,
            offset_id: 0,
            offset_peer: base::InputPeer::InputPeerEmpty,
            done: false,
            widened: false,
        }
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
        let size = if self.widened { PAGE_SIZE } else { self.remaining.min(PAGE_SIZE) };
        (!self.done && self.remaining > 0).then_some(size as i32)
    }

    pub(crate) async fn refill(
        &mut self,
        client: &Client,
        items: Vec<T>,
        last_page: bool,
        next: Option<(i32, i32, i64)>,
    ) -> Result<()> {
        let moved = self.advance(items, last_page, next.map(|(date, id, _)| (date, id)))?;
        if let (true, Some((_, _, chat_id))) = (moved, next) {
            self.offset_peer = client.resolve_peer(chat_id).await?;
        }
        Ok(())
    }

    // Returns whether the date and id moved, so a caller with more cursor fields moves those too.
    pub(crate) fn advance(&mut self, items: Vec<T>, last_page: bool, next: Option<(i32, i32)>) -> Result<bool> {
        self.done = last_page;
        let mut moved = false;
        match next {
            // Same offset again would fetch the same page forever.
            Some(at) if at == (self.offset_date, self.offset_id) => self.done = true,
            Some((date, id)) => {
                self.offset_date = date;
                self.offset_id = id;
                self.widened = false;
                moved = true;
            }
            // A full page of entries with no cursor (communities, deleted topics) has nothing to continue from. Ask
            // again for a whole page, which reaches the entries after them; failing that, say so rather than end the
            // list early.
            None if !last_page && items.is_empty() && !self.widened => self.widened = true,
            None if !last_page && items.is_empty() => {
                return Err(Error::Unexpected("a full page had nothing to continue from"));
            }
            None => self.done = true,
        }
        self.buffer = items.into_iter();
        Ok(moved)
    }
}

#[cfg(test)]
mod tests {
    use super::DialogPages;
    use crate::Client;

    #[tokio::test]
    async fn a_page_without_chats_is_retried_whole() {
        let client = Client::offline();
        let mut pages = DialogPages::<()>::new();
        pages.remaining = 2;
        assert_eq!(pages.limit(), Some(2));

        pages.refill(&client, Vec::new(), false, None).await.unwrap();
        assert_eq!(pages.limit(), Some(100));
        assert!(pages.refill(&client, Vec::new(), false, None).await.is_err());
    }
}
