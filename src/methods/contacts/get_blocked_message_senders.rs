use crate::Client;
use crate::dispatcher::Entities;
use crate::enums::BlockList;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Chat;

#[must_use = "a request does nothing until it is awaited"]
pub struct GetBlockedMessageSenders<'a> {
    client: &'a Client,
    offset: i32,
    limit: i32,
    list: BlockList,
}

impl Client {
    pub fn get_blocked_message_senders(&self, offset: i32, limit: i32) -> GetBlockedMessageSenders<'_> {
        GetBlockedMessageSenders { client: self, offset, limit, list: BlockList::Main }
    }
}

impl GetBlockedMessageSenders<'_> {
    /// Chooses the block list; the default is the main one.
    pub fn block_list(mut self, list: BlockList) -> Self {
        self.list = list;
        self
    }

    async fn send(self) -> Result<Vec<Chat>> {
        let request = functions::contacts::GetBlocked {
            my_stories_from: self.list.my_stories_from(),
            offset: self.offset,
            limit: self.limit,
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
