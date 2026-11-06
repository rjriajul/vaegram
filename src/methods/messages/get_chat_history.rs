use super::pages::{Pages, count};
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Message, PeerRef};

/// Messages of a chat. Awaiting it collects the messages into a `Vec`, up to [`limit`](GetChatHistory::limit);
/// [`next`](GetChatHistory::next) reads them one at a time.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetChatHistory<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    offset_date: i32,
    skip: i32,
    min_id: i32,
    max_id: i32,
    started: bool,
    pages: Pages,
}

impl Client {
    /// Walks the history of `chat_id`, newest message first unless [`reverse`](GetChatHistory::reverse) is set.
    pub fn get_chat_history(&self, chat_id: impl Into<PeerRef>) -> GetChatHistory<'_> {
        GetChatHistory {
            client: self,
            chat_id: chat_id.into(),
            offset_date: 0,
            skip: 0,
            min_id: 0,
            max_id: 0,
            started: false,
            pages: Pages::new(),
        }
    }
}

impl GetChatHistory<'_> {
    /// Stops after this many messages. The default reads the whole history.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Starts below this message id, so the walk skips it and anything newer. With `reverse` it starts at this id.
    pub fn offset_id(mut self, message_id: i32) -> Self {
        self.pages.offset_id = message_id;
        self
    }

    /// Starts at messages older than this Unix time instead of the newest message.
    pub fn offset_date(mut self, unix_time: i32) -> Self {
        self.offset_date = unix_time;
        self
    }

    /// Skips this many messages at the start of the walk: the newest ones, or with `reverse` the oldest. Negative
    /// values step back toward the ones skipped otherwise. It applies to the start only, not to every page.
    pub fn offset(mut self, count: i32) -> Self {
        self.skip = count;
        self
    }

    /// Only messages with an id of at least this one.
    pub fn min_id(mut self, message_id: i32) -> Self {
        self.min_id = message_id;
        self
    }

    /// Only messages with an id of at most this one.
    pub fn max_id(mut self, message_id: i32) -> Self {
        self.max_id = message_id;
        self
    }

    /// Oldest message first, instead of newest first. With `min_id` it starts there; without, at the beginning of
    /// the chat.
    pub fn reverse(mut self) -> Self {
        self.pages.reverse = true;
        self
    }

    /// How many messages the chat has, whatever `limit` says: the count Telegram gave with the last page, or,
    /// before any page, from one request for a single message. Either way it's asked for once.
    pub async fn total(&mut self) -> Result<i32> {
        if let Some(total) = self.pages.total {
            return Ok(total);
        }
        let (chat_id, peer) = self.client.resolve(&self.chat_id).await?;
        self.chat_id = PeerRef::Id(chat_id);
        let request = functions::messages::GetHistory {
            peer,
            offset_id: 0,
            offset_date: self.offset_date,
            add_offset: 0,
            limit: 1,
            max_id: self.exclusive_max(),
            min_id: self.exclusive_min(),
            hash: 0,
        };
        let total = count(&self.client.invoke(&request).await?);
        self.pages.total = Some(total);
        Ok(total)
    }

    /// Returns the next message, or `None` at the end of the history.
    pub async fn next(&mut self) -> Result<Option<Message>> {
        loop {
            if let Some(message) = self.pages.pop(self.client) {
                return Ok(Some(message));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let (chat_id, peer) = self.client.resolve(&self.chat_id).await?;
            self.chat_id = PeerRef::Id(chat_id);
            let (min_id, max_id) = (self.exclusive_min(), self.exclusive_max());
            let skip = if self.started { 0 } else { self.skip };
            // Telegram only counts from an offset id, so a bound has to become one. Newest first starts below the
            // newest allowed id. Oldest first starts at the lowest allowed one and moves the window `limit`
            // messages toward the newer ones, which is where they begin, and `skip` further to leave the oldest
            // out.
            let toward_newer = skip.saturating_add(limit).saturating_neg();
            let (offset_id, add_offset) = match (self.pages.reverse, self.pages.offset_id) {
                (false, 0) => (max_id, skip),
                (false, offset_id) => (offset_id, skip),
                (true, 0) => (min_id.max(1), toward_newer),
                (true, offset_id) => (offset_id, toward_newer),
            };
            let request = functions::messages::GetHistory {
                peer,
                offset_id,
                offset_date: self.offset_date,
                add_offset,
                limit,
                max_id,
                min_id,
                hash: 0,
            };
            let page = self.client.invoke(&request).await?;
            self.started = true;
            self.pages.fill(self.client, page);
            self.pages.prefetch(self.client).await;
        }
    }

    // Telegram's own bounds leave the id out, so the inclusive ones asked for here move by one.
    fn exclusive_min(&self) -> i32 {
        if self.min_id > 0 { self.min_id - 1 } else { 0 }
    }

    fn exclusive_max(&self) -> i32 {
        if self.max_id > 0 { self.max_id.saturating_add(1) } else { 0 }
    }
}

pager!(GetChatHistory, Message);

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc::UnboundedReceiver;

    use super::super::pages::tests::slice as page;
    use crate::Client;
    use crate::raw::functions::messages::GetHistory;
    use crate::raw::{Serializable, base, types};

    /// The request for chat 7 with these `(offset_id, add_offset, limit, min_id, max_id)`.
    fn asked(offset_id: i32, add_offset: i32, limit: i32, min_id: i32, max_id: i32) -> Vec<u8> {
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        let peer = base::InputPeer::InputPeerUser(Box::new(user));
        GetHistory { peer, offset_id, offset_date: 0, add_offset, limit, max_id, min_id, hash: 0 }.to_bytes()
    }

    fn sent(rx: &mut UnboundedReceiver<Vec<u8>>) -> Vec<Vec<u8>> {
        std::iter::from_fn(|| rx.try_recv().ok()).collect()
    }

    fn chat_7(replies: Vec<Vec<u8>>) -> (Client, UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::scripted(replies.into_iter().map(Ok).collect());
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        (client, rx)
    }

    #[tokio::test]
    async fn total_reuses_the_fetched_page() {
        let (client, mut rx) = chat_7(vec![page(&[3, 2], 40)]);
        let mut messages = client.get_chat_history(7).limit(2);
        assert_eq!(messages.next().await.unwrap().map(|m| m.id), Some(3));
        assert_eq!(messages.total().await.unwrap(), 40);
        assert_eq!(messages.total().await.unwrap(), 40);
        assert_eq!(sent(&mut rx).len(), 1, "total asked again");
    }

    #[tokio::test]
    async fn total_before_next_makes_one_request() {
        let (client, mut rx) = chat_7(vec![page(&[3], 40)]);
        let mut messages = client.get_chat_history(7).min_id(5).max_id(9);
        assert_eq!(messages.total().await.unwrap(), 40);
        assert_eq!(messages.total().await.unwrap(), 40);
        assert_eq!(sent(&mut rx), [asked(0, 0, 1, 4, 10)]);
    }

    #[tokio::test]
    async fn bounds_are_inclusive_and_newest_first_starts_below_the_top() {
        let (client, mut rx) = chat_7(vec![page(&[7, 6], 2)]);
        let mut messages = client.get_chat_history(7).min_id(6).max_id(8).offset(2);
        assert_eq!(messages.next().await.unwrap().map(|m| m.id), Some(7));
        assert_eq!(sent(&mut rx), [asked(9, 2, 100, 5, 9)]);
    }

    #[tokio::test]
    async fn reverse_reads_oldest_first_and_moves_up_by_id() {
        // Telegram answers each page newest first; the first holds the two oldest messages.
        let (client, mut rx) = chat_7(vec![page(&[2, 1], 3), page(&[3], 3), page(&[], 3)]);
        let mut messages = client.get_chat_history(7).reverse();
        let mut ids = Vec::new();
        while let Some(m) = messages.next().await.unwrap() {
            ids.push(m.id);
        }
        assert_eq!(ids, [1, 2, 3]);
        // The window starts at the oldest id and looks a whole page back from it; it then moves to just past the
        // newest id read.
        let expected = [asked(1, -100, 100, 0, 0), asked(3, -100, 100, 0, 0), asked(4, -100, 100, 0, 0)];
        assert_eq!(sent(&mut rx), expected);
    }

    #[tokio::test]
    async fn reverse_with_min_id_starts_there() {
        let (client, mut rx) = chat_7(vec![page(&[], 0)]);
        let mut messages = client.get_chat_history(7).reverse().min_id(10).limit(5);
        assert!(messages.next().await.unwrap().is_none());
        assert_eq!(sent(&mut rx), [asked(9, -5, 5, 9, 0)]);
    }

    /// What Telegram answers to `getHistory`, newest first, for a chat holding `chat`: the window starts
    /// `add_offset` messages after the first one older than `offset_id`, and negative values reach newer ones.
    fn window(chat: &[i32], offset_id: i32, add_offset: i32, limit: i32) -> Vec<i32> {
        let mut newest_first = chat.to_vec();
        newest_first.sort_by(|a, b| b.cmp(a));
        let older = newest_first.iter().position(|&id| id < offset_id).unwrap_or(newest_first.len()) as i64;
        // The window is `limit` messages from `start`; whatever part of it lies past either end of the chat is lost.
        let (start, len) = (older + i64::from(add_offset), newest_first.len() as i64);
        let (from, to) = (start.clamp(0, len) as usize, (start + i64::from(limit)).clamp(0, len) as usize);
        newest_first[from..to].to_vec()
    }

    #[tokio::test]
    async fn reverse_with_an_offset_skips_the_oldest() {
        let chat = [1, 2, 3, 4, 5];
        // Skipping 2 and taking 2 of a chat of 5 oldest first is messages 3 and 4. The request below has to be the
        // one that asks Telegram for exactly those, so the answer the script gives is the one Telegram would.
        let asking = asked(1, -(2 + 2), 2, 0, 0);
        assert_eq!(window(&chat, 1, -(2 + 2), 2), [4, 3]);
        let (client, mut rx) = chat_7(vec![page(&window(&chat, 1, -(2 + 2), 2), 5)]);
        let mut messages = client.get_chat_history(7).reverse().offset(2).limit(2);
        let mut ids = Vec::new();
        while let Some(m) = messages.next().await.unwrap() {
            ids.push(m.id);
        }
        assert_eq!(ids, [3, 4]);
        assert_eq!(sent(&mut rx), [asking]);
        // Without the skip the same window would start at the oldest.
        assert_eq!(window(&chat, 1, -2, 2), [2, 1]);
    }

    #[tokio::test]
    async fn offsets_and_bounds_at_the_edge_of_i32_do_not_overflow() {
        let (client, mut rx) = chat_7(vec![page(&[], 0), page(&[], 0)]);
        let mut oldest = client.get_chat_history(7).reverse().offset(i32::MAX).min_id(i32::MAX);
        assert!(oldest.next().await.unwrap().is_none());
        let mut newest = client.get_chat_history(7).offset(i32::MIN).max_id(i32::MAX);
        assert!(newest.next().await.unwrap().is_none());
        let expected =
            [asked(i32::MAX - 1, i32::MIN + 1, 100, i32::MAX - 1, 0), asked(i32::MAX, i32::MIN, 100, 0, i32::MAX)];
        assert_eq!(sent(&mut rx), expected);
    }
}
