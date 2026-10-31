use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{ChatSettings, PeerRef};

impl Client {
    pub async fn get_chat_settings(&self, chat_id: impl Into<PeerRef>) -> Result<ChatSettings> {
        let peer = self.resolve_peer(chat_id).await?;
        let base::messages::PeerSettings::PeerSettings(reply) =
            self.invoke(&functions::messages::GetPeerSettings { peer }).await?;
        let reply = *reply;
        let entities = Entities::new(self, reply.users, reply.chats);
        let base::PeerSettings::PeerSettings(settings) = reply.settings;
        Ok(ChatSettings::parse(*settings, &entities))
    }
}
