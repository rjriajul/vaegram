use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{Chat, PeerRef};
use crate::utils::{get_chat_id, get_peer};

impl Client {
    pub async fn get_common_chats(&self, user_id: impl Into<PeerRef>) -> Result<Vec<Chat>> {
        let user_id = self.resolve_user(&user_id.into()).await?;
        let request = functions::messages::GetCommonChats { user_id, max_id: 0, limit: 100 };
        let chats = match self.invoke(&request).await? {
            base::messages::Chats::Chats(c) => c.chats,
            base::messages::Chats::ChatsSlice(c) => c.chats,
        };
        let ids: Vec<i64> = chats.iter().filter_map(get_chat_id).collect();
        let entities = Entities::new(self, Vec::new(), chats);
        Ok(ids.into_iter().map(|id| Chat::parse(&get_peer(id), &entities, false)).collect())
    }
}
