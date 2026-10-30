use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{BoostsStatus, PeerRef};

impl Client {
    pub async fn get_boosts_status(&self, chat_id: impl Into<PeerRef>) -> Result<BoostsStatus> {
        let peer = self.resolve_peer(chat_id).await?;
        let base::premium::BoostsStatus::BoostsStatus(status) =
            self.invoke(&functions::premium::GetBoostsStatus { peer }).await?;
        Ok(BoostsStatus::parse(*status))
    }
}
