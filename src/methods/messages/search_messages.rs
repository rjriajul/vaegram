use super::pages::{Pages, count};
use crate::Client;
use crate::enums::MessagesFilter;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Message, PeerRef};

/// Messages found by [`Client::search_messages`]. Call [`next`](SearchMessages::next) until it returns `None`.
#[must_use = "call .next() to fetch messages"]
pub struct SearchMessages<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    query: String,
    from_user: Option<PeerRef>,
    filter: MessagesFilter,
    pages: Pages,
}

impl Client {
    /// Searches the messages of one chat, newest first.
    ///
    /// Without [`query`](SearchMessages::query), [`from_user`](SearchMessages::from_user) or [`filter`](SearchMessages::filter) it walks the whole history.
    pub fn search_messages(&self, chat_id: impl Into<PeerRef>) -> SearchMessages<'_> {
        SearchMessages {
            client: self,
            chat_id: chat_id.into(),
            query: String::new(),
            from_user: None,
            filter: MessagesFilter::Empty,
            pages: Pages::new(),
        }
    }
}

impl SearchMessages<'_> {
    /// Only messages containing this text.
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
        self
    }

    /// Only messages sent by this user.
    pub fn from_user(mut self, user_id: impl Into<PeerRef>) -> Self {
        self.from_user = Some(user_id.into());
        self
    }

    /// Only messages of this kind.
    pub fn filter(mut self, filter: MessagesFilter) -> Self {
        self.filter = filter;
        self
    }

    /// Stops after this many messages. Without it the search runs to the oldest match.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// How many messages match, whatever `limit` says: the count Telegram gave with the last page, or, before any
    /// page, from one request for a single message. Either way it's asked for once.
    pub async fn total(&mut self) -> Result<i32> {
        if let Some(total) = self.pages.total {
            return Ok(total);
        }
        let request = self.request(1, false).await?;
        let total = count(&self.client.invoke(&request).await?);
        self.pages.total = Some(total);
        Ok(total)
    }

    /// Returns the next match, fetching a page when the buffered one is used up. `None` after the last match.
    pub async fn next(&mut self) -> Result<Option<Message>> {
        loop {
            if let Some(message) = self.pages.pop(self.client) {
                return Ok(Some(message));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let request = self.request(limit, true).await?;
            let page = self.client.invoke(&request).await?;
            self.pages.fill(self.client, page);
            self.pages.prefetch(self.client).await;
        }
    }

    /// `paged` continues from the last page; without it the request starts at the newest message.
    async fn request(&mut self, limit: i32, paged: bool) -> Result<functions::messages::Search> {
        let from_id = match &self.from_user {
            Some(user) => {
                let (user_id, peer) = self.client.resolve(user).await?;
                self.from_user = Some(PeerRef::Id(user_id));
                Some(peer)
            }
            None => None,
        };
        let (chat_id, peer) = self.client.resolve(&self.chat_id).await?;
        self.chat_id = PeerRef::Id(chat_id);
        Ok(functions::messages::Search {
            peer,
            q: self.query.clone(),
            from_id,
            saved_peer_id: None,
            saved_reaction: None,
            top_msg_id: None,
            filter: self.filter.write(),
            min_date: 0,
            max_date: 0,
            offset_id: if paged { self.pages.offset_id } else { 0 },
            add_offset: 0,
            limit,
            max_id: 0,
            min_id: 0,
            hash: 0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::pages::tests::slice;
    use crate::Client;
    use crate::enums::MessagesFilter;
    use crate::raw::{base, types};

    #[tokio::test]
    async fn sends_the_filter() {
        let (client, mut rx) = Client::recording();
        let _ = client.search_messages(-5).filter(MessagesFilter::VideoNote).next().await;
        let body = rx.try_recv().expect("no request sent");
        let round_video = 0xb549da53u32.to_le_bytes();
        assert!(body.windows(4).any(|w| w == round_video));
    }

    fn chat_7(replies: Vec<Vec<u8>>) -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (client, rx) = Client::scripted(replies.into_iter().map(Ok).collect());
        let user = types::InputPeerUser { user_id: 7, access_hash: 1 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(user)));
        (client, rx)
    }

    #[tokio::test]
    async fn total_reuses_the_fetched_page() {
        let (client, mut rx) = chat_7(vec![slice(&[5, 4], 12)]);
        let mut found = client.search_messages(7).query("x").limit(1);
        assert_eq!(found.next().await.unwrap().map(|m| m.id), Some(5));
        assert_eq!(found.total().await.unwrap(), 12);
        assert_eq!(found.total().await.unwrap(), 12);
        assert!(rx.try_recv().is_ok() && rx.try_recv().is_err(), "total asked again");
    }

    #[tokio::test]
    async fn total_before_next_makes_one_request() {
        let (client, mut rx) = chat_7(vec![slice(&[5], 12)]);
        let mut found = client.search_messages(7).query("x");
        assert_eq!(found.total().await.unwrap(), 12);
        assert_eq!(found.total().await.unwrap(), 12);
        let body = rx.try_recv().expect("no request sent");
        // The request ends with `limit`, `max_id` and `min_id` (4 bytes each) and `hash` (8).
        assert_eq!(body[body.len() - 20..body.len() - 16], 1i32.to_le_bytes());
        assert!(rx.try_recv().is_err(), "total asked twice");
    }
}
