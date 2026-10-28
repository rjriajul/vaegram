use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::StarAmount;

impl Client {
    pub async fn get_business_account_star_balance(&self, business_connection_id: &str) -> Result<StarAmount> {
        let request = functions::payments::GetStarsStatus { ton: false, peer: base::InputPeer::InputPeerSelf };
        let (base::payments::StarsStatus::StarsStatus(status), _) =
            self.invoke_business(Some(business_connection_id), request).await?;
        Ok(StarAmount::parse(&status.balance))
    }
}
