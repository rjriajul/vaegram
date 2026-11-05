use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::base;
use crate::types::Message;
use crate::utils::get_peer_id;

const HISTORY_LIMIT: usize = 100;

/// Cross-chat searches page by Telegram's `next_rate`, because message ids aren't ordered across chats.
pub(super) struct Pages {
    buffer: std::vec::IntoIter<base::Message>,
    entities: Entities,
    // The page parsed ahead of time, when a fetch option needs whole messages; `pop` takes from it first.
    prefetched: Option<std::vec::IntoIter<Message>>,
    pub(super) remaining: usize,
    by_rate: bool,
    // Oldest first: `offset_id` is then the lowest id the next page may hold, not the last one seen.
    pub(super) reverse: bool,
    // The count of the last page, or what a one-message request said before any.
    pub(super) total: Option<i32>,
    pub(super) offset_rate: i32,
    offset_peer: i64,
    pub(super) offset_id: i32,
    done: bool,
}

impl Pages {
    pub(super) fn new() -> Pages {
        Pages {
            buffer: Vec::new().into_iter(),
            entities: Entities::default(),
            prefetched: None,
            remaining: usize::MAX,
            by_rate: false,
            reverse: false,
            total: None,
            offset_rate: 0,
            offset_peer: 0,
            offset_id: 0,
            done: false,
        }
    }

    pub(super) fn by_rate() -> Pages {
        Pages { by_rate: true, ..Pages::new() }
    }

    pub(super) fn limit(&self) -> Option<i32> {
        (!self.done && self.remaining > 0).then(|| self.remaining.min(HISTORY_LIMIT) as i32)
    }

    pub(super) async fn offset_peer(&self, client: &Client) -> Result<base::InputPeer> {
        match self.offset_peer {
            0 => Ok(base::InputPeer::InputPeerEmpty),
            chat_id => client.resolve_peer(chat_id).await,
        }
    }

    pub(super) fn fill(&mut self, client: &Client, page: base::messages::Messages) {
        let (next_rate, complete) = match &page {
            base::messages::Messages::Messages(_) | base::messages::Messages::MessagesNotModified(_) => (None, true),
            base::messages::Messages::MessagesSlice(m) => (m.next_rate, false),
            base::messages::Messages::ChannelMessages(_) => (None, false),
        };
        // A plain page holds everything the search found, so its length is the total only when it comes first: after
        // a slice it's just the last few.
        if !(matches!(page, base::messages::Messages::Messages(_)) && self.total.is_some()) {
            self.total = Some(count(&page));
        }
        let (mut messages, entities) = Message::unpack(client, page);
        // Telegram answers newest first; the oldest-first walk reads each page from its far end.
        if self.reverse {
            messages.reverse();
        }
        if self.advance(messages.last().map(position), next_rate, complete) {
            self.buffer = messages.into_iter();
            self.entities = entities;
            self.prefetched = None;
        }
    }

    /// Right after `fill`: with a fetch option on, parses the page now and fills what the options ask for in one
    /// batch for the whole page.
    pub(super) async fn prefetch(&mut self, client: &Client) {
        if !client.0.fetch.any() {
            return;
        }
        let raw = std::mem::take(&mut self.buffer);
        let mut messages: Vec<Message> = raw.filter_map(|m| Message::from_raw(client, m, &self.entities)).collect();
        client.prefetch(&mut messages).await;
        self.prefetched = Some(messages.into_iter());
    }

    fn advance(&mut self, last: Option<(i64, i32, i32)>, next_rate: Option<i32>, complete: bool) -> bool {
        let Some((peer, id, date)) = last else {
            self.done = true;
            return false;
        };
        if self.by_rate {
            // Without a next_rate, Telegram says to go on from the date of the last message.
            let rate = next_rate.unwrap_or(date);
            if (rate, peer, id) == (self.offset_rate, self.offset_peer, self.offset_id) {
                self.done = true;
                return false;
            }
            self.offset_rate = rate;
        } else if self.reverse {
            // The offset is the lowest id the next page may hold. A page that ends below it moved backwards.
            if id < self.offset_id {
                self.done = true;
                return false;
            }
            (self.offset_peer, self.offset_id) = (peer, id.saturating_add(1));
            self.done |= complete;
            return true;
        } else if self.offset_id != 0 && id >= self.offset_id {
            // An offset that doesn't move down means Telegram has nothing older to give.
            self.done = true;
            return false;
        }
        (self.offset_peer, self.offset_id) = (peer, id);
        self.done |= complete;
        true
    }

    pub(super) fn pop(&mut self, client: &Client) -> Option<Message> {
        if let Some(prefetched) = self.prefetched.as_mut().filter(|_| self.remaining > 0) {
            let message = prefetched.next()?;
            self.remaining -= 1;
            return Some(message);
        }
        while self.remaining > 0 {
            if let Some(message) = Message::from_raw(client, self.buffer.next()?, &self.entities) {
                self.remaining -= 1;
                return Some(message);
            }
        }
        None
    }
}

pub(super) fn count(page: &base::messages::Messages) -> i32 {
    match page {
        base::messages::Messages::Messages(m) => m.messages.len() as i32,
        base::messages::Messages::MessagesSlice(m) => m.count,
        base::messages::Messages::ChannelMessages(m) => m.count,
        base::messages::Messages::MessagesNotModified(m) => m.count,
    }
}

fn position(message: &base::Message) -> (i64, i32, i32) {
    match message {
        base::Message::MessageEmpty(m) => (m.peer_id.as_ref().map_or(0, get_peer_id), m.id, 0),
        base::Message::Message(m) => (get_peer_id(&m.peer_id), m.id, m.date),
        base::Message::MessageService(m) => (get_peer_id(&m.peer_id), m.id, m.date),
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::Pages;
    use crate::Client;
    use crate::raw::{Deserializable, Serializable, base, types};
    use crate::types::Message;
    use crate::utils::get_peer;

    /// A `messagesSlice` the way Telegram answers: newest first, `count` for everything that matches, and every
    /// message in chat 7.
    pub(in crate::methods::messages) fn slice(ids: &[i32], count: i32) -> Vec<u8> {
        let message = |&id: &i32| base::Message::Message(Box::new(Message::bare(id, get_peer(7), id)));
        let slice = types::messages::MessagesSlice {
            inexact: false,
            count,
            next_rate: None,
            offset_id_offset: None,
            search_flood: None,
            messages: ids.iter().map(message).collect(),
            topics: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
        };
        base::messages::Messages::MessagesSlice(Box::new(slice)).to_bytes()
    }

    #[test]
    fn a_plain_page_after_the_first_keeps_the_total() {
        let message = |id| base::Message::Message(Box::new(Message::bare(id, get_peer(7), id)));
        let plain = |ids: &[i32]| {
            let page = types::messages::Messages {
                messages: ids.iter().map(|&id| message(id)).collect(),
                topics: Vec::new(),
                chats: Vec::new(),
                users: Vec::new(),
            };
            base::messages::Messages::Messages(Box::new(page))
        };
        let client = Client::offline();
        let mut pages = Pages::new();
        pages.fill(&client, plain(&[9, 8, 7]));
        assert_eq!(pages.total, Some(3));

        let mut pages = Pages::new();
        let slice = base::messages::Messages::deserialize(&mut &slice(&[9, 8], 40)[..]).unwrap();
        pages.fill(&client, slice);
        pages.fill(&client, plain(&[7]));
        assert_eq!(pages.total, Some(40));
    }

    #[test]
    fn oldest_first_pages_move_up_and_stop_when_they_move_back() {
        let mut pages = Pages::new();
        pages.reverse = true;
        assert!(pages.advance(Some((7, 2, 1)), None, false));
        assert_eq!(pages.offset_id, 3);
        assert!(pages.advance(Some((7, 9, 1)), None, false));
        assert_eq!(pages.offset_id, 10);
        assert!(!pages.advance(Some((7, 4, 1)), None, false));
        assert_eq!(pages.limit(), None);
    }

    #[test]
    fn rate_pages_go_on_from_the_last_date_without_a_next_rate() {
        let mut pages = Pages::by_rate();
        assert!(pages.advance(Some((-1001, 50, 111)), Some(7), false));
        assert_eq!((pages.offset_rate, pages.offset_peer, pages.offset_id), (7, -1001, 50));
        assert!(pages.limit().is_some());
        assert!(pages.advance(Some((42, 900, 222)), None, false));
        assert_eq!((pages.offset_rate, pages.offset_peer, pages.offset_id), (222, 42, 900));
        assert!(pages.limit().is_some());
        assert!(!pages.advance(Some((42, 900, 222)), None, false));
        assert_eq!(pages.limit(), None);
    }

    #[test]
    fn id_pages_stop_when_the_offset_stalls() {
        let mut pages = Pages::new();
        assert!(pages.advance(Some((-5, 300, 1)), None, false));
        assert!(pages.advance(Some((-5, 200, 1)), None, false));
        assert_eq!(pages.offset_id, 200);
        assert!(!pages.advance(Some((-5, 200, 1)), None, false));
        assert_eq!(pages.limit(), None);
    }

    #[test]
    fn complete_or_empty_answers_end_the_walk() {
        let mut pages = Pages::new();
        assert!(pages.advance(Some((1, 10, 1)), None, true));
        assert_eq!(pages.limit(), None);

        let mut pages = Pages::by_rate();
        assert!(!pages.advance(None, Some(3), false));
        assert_eq!(pages.limit(), None);
    }

    #[test]
    fn limit_caps_each_page() {
        let mut pages = Pages::new();
        assert_eq!(pages.limit(), Some(100));
        pages.remaining = 30;
        assert_eq!(pages.limit(), Some(30));
        pages.remaining = 0;
        assert_eq!(pages.limit(), None);
    }
}
