use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{Chat, PeerRef};
use crate::utils::{get_chat_id, get_peer};

impl Client {
    pub async fn get_similar_channels(&self, chat_id: impl Into<PeerRef>) -> Result<Vec<Chat>> {
        let channel = Some(self.resolve_channel(&chat_id.into()).await?);
        let found = self.invoke(&functions::channels::GetChannelRecommendations { channel }).await?;
        Ok(parse_channels(self, found))
    }
}

pub(super) fn parse_channels(client: &Client, found: base::messages::Chats) -> Vec<Chat> {
    let chats = match found {
        base::messages::Chats::Chats(c) => c.chats,
        base::messages::Chats::ChatsSlice(c) => c.chats,
    };
    let ids: Vec<i64> = chats.iter().filter_map(get_chat_id).collect();
    let entities = Entities::new(client, Vec::new(), chats);
    ids.iter().map(|&id| Chat::parse(&get_peer(id), &entities, false)).collect()
}
