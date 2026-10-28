use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::StarAmount;

impl Client {
    pub async fn get_my_star_balance(&self) -> Result<StarAmount> {
        let request = functions::payments::GetStarsStatus { ton: false, peer: base::InputPeer::InputPeerSelf };
        let base::payments::StarsStatus::StarsStatus(status) = self.invoke(&request).await?;
        Ok(StarAmount::parse(&status.balance))
    }
}
