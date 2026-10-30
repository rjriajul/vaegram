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
    pub(super) remaining: usize,
    by_rate: bool,
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
            remaining: usize::MAX,
            by_rate: false,
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
        let (messages, entities) = Message::unpack(client, page);
        if self.advance(messages.last().map(position), next_rate, complete) {
            self.buffer = messages.into_iter();
            self.entities = entities;
        }
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
mod tests {
    use super::Pages;

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
