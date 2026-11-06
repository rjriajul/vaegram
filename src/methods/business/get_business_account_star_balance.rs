use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::StarAmount;

impl Client {
    /// Returns the Stars balance of the business account behind a connection.
    pub async fn get_business_account_star_balance(
        &self,
        business_connection_id: impl Into<String>,
    ) -> Result<StarAmount> {
        let business_connection_id: String = business_connection_id.into();
        let request = functions::payments::GetStarsStatus { ton: false, peer: base::InputPeer::InputPeerSelf };
        let (base::payments::StarsStatus::StarsStatus(status), _) =
            self.invoke_business(Some(&business_connection_id), request).await?;
        Ok(StarAmount::parse(&status.balance))
    }
}
