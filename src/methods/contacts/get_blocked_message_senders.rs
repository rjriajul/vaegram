use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Chat;

impl Client {
    pub async fn get_blocked_message_senders(&self, offset: i32, limit: i32) -> Result<Vec<Chat>> {
        let request = functions::contacts::GetBlocked { my_stories_from: false, offset, limit };
        let (blocked, users, chats) = match self.invoke(&request).await? {
            base::contacts::Blocked::Blocked(b) => (b.blocked, b.users, b.chats),
            base::contacts::Blocked::BlockedSlice(b) => (b.blocked, b.users, b.chats),
        };
        let entities = Entities::new(self, users, chats);
        Ok(blocked.iter().map(|base::PeerBlocked::PeerBlocked(b)| Chat::parse(&b.peer_id, &entities, false)).collect())
    }
}
