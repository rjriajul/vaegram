use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::BlockList;
use crate::errors::Result;
use crate::methods::pages::OffsetPages;
use crate::raw::{base, functions};
use crate::types::Chat;

/// Users and chats on a block list, built by [`Client::get_blocked_message_senders`].
///
/// Awaiting it collects them into a `Vec`, up to [`limit`](GetBlockedMessageSenders::limit);
/// [`next`](GetBlockedMessageSenders::next) reads them one at a time. They are fetched 100 to a page.
#[must_use = "a pager does nothing until it is awaited or read with next()"]
pub struct GetBlockedMessageSenders<'a> {
    client: &'a Client,
    list: BlockList,
    pages: OffsetPages<Chat, i32>,
}

impl Client {
    /// Lists the users and chats on a block list.
    ///
    /// Reads [`BlockList::Main`] unless [`block_list`](GetBlockedMessageSenders::block_list) picks another.
    pub fn get_blocked_message_senders(&self) -> GetBlockedMessageSenders<'_> {
        GetBlockedMessageSenders { client: self, list: BlockList::Main, pages: OffsetPages::starting_at(100, 0) }
    }
}

impl GetBlockedMessageSenders<'_> {
    /// Chooses the block list. Defaults to [`BlockList::Main`].
    pub fn block_list(mut self, list: BlockList) -> Self {
        self.list = list;
        self
    }

    /// Skips this many entries from the start of the list.
    pub fn offset(mut self, offset: i32) -> Self {
        self.pages.set_offset(offset);
        self
    }

    /// Stops after this many entries in total. The default reads the whole list.
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    /// Returns the next blocked user or chat, or `None` once the list or the limit is exhausted.
    pub async fn next(&mut self) -> Result<Option<Chat>> {
        loop {
            if let Some(chat) = self.pages.pop() {
                return Ok(Some(chat));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let offset = self.pages.offset().copied().unwrap_or(0);
            let request =
                functions::contacts::GetBlocked { my_stories_from: self.list.my_stories_from(), offset, limit };
            let (total, blocked, users, chats) = match self.client.invoke(&request).await? {
                // A full list means nothing follows, so the total is everything up to its end.
                base::contacts::Blocked::Blocked(b) => {
                    (offset.saturating_add(b.blocked.len() as i32), b.blocked, b.users, b.chats)
                }
                base::contacts::Blocked::BlockedSlice(b) => (b.count, b.blocked, b.users, b.chats),
            };
            let entities = Entities::new(self.client, users, chats);
            let parsed =
                blocked.iter().map(|base::PeerBlocked::PeerBlocked(b)| Chat::parse(&b.peer_id, &entities, false));
            self.pages.push_counted(parsed.collect(), blocked.len(), total);
        }
    }
}

pager!(GetBlockedMessageSenders, Chat);

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::enums::BlockList;
    use crate::raw::functions::contacts::GetBlocked;
    use crate::raw::{Serializable, base, types};

    fn blocked(ids: &[i64]) -> Vec<base::PeerBlocked> {
        let peer = |id| base::Peer::PeerUser(types::PeerUser { user_id: id });
        ids.iter()
            .map(|&id| base::PeerBlocked::PeerBlocked(Box::new(types::PeerBlocked { peer_id: peer(id), date: 0 })))
            .collect()
    }

    fn slice(count: i32, ids: &[i64]) -> crate::Result<Vec<u8>> {
        let slice =
            types::contacts::BlockedSlice { count, blocked: blocked(ids), chats: Vec::new(), users: Vec::new() };
        Ok(base::contacts::Blocked::BlockedSlice(Box::new(slice)).to_bytes())
    }

    fn whole(ids: &[i64]) -> crate::Result<Vec<u8>> {
        let whole = types::contacts::Blocked { blocked: blocked(ids), chats: Vec::new(), users: Vec::new() };
        Ok(base::contacts::Blocked::Blocked(Box::new(whole)).to_bytes())
    }

    fn request(offset: i32, limit: i32) -> Vec<u8> {
        GetBlocked { my_stories_from: false, offset, limit }.to_bytes()
    }

    #[tokio::test]
    async fn pages_skip_what_was_read_until_the_count() {
        let (client, mut sent) = Client::scripted(vec![slice(3, &[1, 2]), slice(3, &[3])]);
        let chats = client.get_blocked_message_senders().await.unwrap();
        assert_eq!(chats.iter().map(|c| c.id).collect::<Vec<_>>(), [1, 2, 3]);
        assert_eq!(sent.try_recv().unwrap(), request(0, 100));
        assert_eq!(sent.try_recv().unwrap(), request(2, 100));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn offset_and_limit_reach_the_request_and_a_whole_list_ends_it() {
        let (client, mut sent) = Client::scripted(vec![whole(&[4]), slice(9, &[5, 6])]);
        let chats = client.get_blocked_message_senders().offset(5).limit(10).await.unwrap();
        assert_eq!(chats.iter().map(|c| c.id).collect::<Vec<_>>(), [4]);
        assert_eq!(sent.try_recv().unwrap(), request(5, 10));
        assert!(sent.try_recv().is_err());

        let mut stories = client.get_blocked_message_senders().block_list(BlockList::Stories).limit(1);
        assert_eq!(stories.next().await.unwrap().map(|c| c.id), Some(5));
        assert_eq!(stories.next().await.unwrap().map(|c| c.id), None);
        assert_eq!(sent.try_recv().unwrap(), GetBlocked { my_stories_from: true, offset: 0, limit: 1 }.to_bytes());
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn an_empty_list_sends_one_request() {
        let (client, mut sent) = Client::scripted(vec![slice(0, &[])]);
        assert!(client.get_blocked_message_senders().await.unwrap().is_empty());
        assert_eq!(sent.try_recv().unwrap(), request(0, 100));
        assert!(sent.try_recv().is_err());
    }
}
