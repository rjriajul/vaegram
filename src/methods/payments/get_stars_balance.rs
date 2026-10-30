use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{PeerRef, StarAmount};

impl Client {
    pub async fn get_stars_balance(&self, chat_id: impl Into<PeerRef>) -> Result<StarAmount> {
        let peer = self.resolve_peer(chat_id).await?;
        let request = functions::payments::GetStarsStatus { ton: false, peer };
        let base::payments::StarsStatus::StarsStatus(status) = self.invoke(&request).await?;
        Ok(StarAmount::parse(&status.balance))
    }
}
