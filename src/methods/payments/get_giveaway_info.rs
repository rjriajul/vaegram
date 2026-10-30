use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{GiveawayInfo, PeerRef};

impl Client {
    pub async fn get_giveaway_info(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> Result<GiveawayInfo> {
        let peer = self.resolve_peer(chat_id).await?;
        let info = self.invoke(&functions::payments::GetGiveawayInfo { peer, msg_id: message_id }).await?;
        Ok(GiveawayInfo::parse(info))
    }
}
