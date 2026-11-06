use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::BlockList;
use crate::errors::Result;
use crate::methods::pages::wire_limit;
use crate::raw::{base, functions};
use crate::types::Chat;

/// Request built by [`Client::get_blocked_message_senders`].
#[must_use = "a request does nothing until it is awaited"]
pub struct GetBlockedMessageSenders<'a> {
    client: &'a Client,
    offset: i32,
    limit: usize,
    list: BlockList,
}

impl Client {
    /// Returns up to `limit` users and chats from a block list, skipping the first `offset`.
    ///
    /// Reads [`BlockList::Main`] unless [`block_list`](GetBlockedMessageSenders::block_list) picks another.
    pub fn get_blocked_message_senders(&self, offset: i32, limit: usize) -> GetBlockedMessageSenders<'_> {
        GetBlockedMessageSenders { client: self, offset, limit, list: BlockList::Main }
    }
}

impl GetBlockedMessageSenders<'_> {
    /// Chooses the block list. Defaults to [`BlockList::Main`].
    pub fn block_list(mut self, list: BlockList) -> Self {
        self.list = list;
        self
    }

    async fn send(self) -> Result<Vec<Chat>> {
        let request = functions::contacts::GetBlocked {
            my_stories_from: self.list.my_stories_from(),
            offset: self.offset,
            limit: wire_limit(self.limit),
        };
        let (blocked, users, chats) = match self.client.invoke(&request).await? {
            base::contacts::Blocked::Blocked(b) => (b.blocked, b.users, b.chats),
            base::contacts::Blocked::BlockedSlice(b) => (b.blocked, b.users, b.chats),
        };
        let entities = Entities::new(self.client, users, chats);
        Ok(blocked.iter().map(|base::PeerBlocked::PeerBlocked(b)| Chat::parse(&b.peer_id, &entities, false)).collect())
    }
}

into_future!(GetBlockedMessageSenders, Vec<Chat>);
