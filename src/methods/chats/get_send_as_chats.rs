use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{Chat, PeerRef};

impl Client {
    pub async fn get_send_as_chats(&self, chat_id: impl Into<PeerRef>) -> Result<Vec<Chat>> {
        let peer = self.resolve_peer(chat_id).await?;
        let request = functions::channels::GetSendAs { for_paid_reactions: false, for_live_stories: false, peer };
        let base::channels::SendAsPeers::SendAsPeers(r) = self.invoke(&request).await?;
        let entities = Entities::new(self, r.users, r.chats);
        Ok(r.peers.iter().map(|base::SendAsPeer::SendAsPeer(p)| Chat::parse(&p.peer, &entities, false)).collect())
    }
}
