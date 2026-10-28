use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn refund_star_payment(
        &self,
        user_id: impl Into<PeerRef>,
        telegram_payment_charge_id: &str,
    ) -> Result<()> {
        let request = functions::payments::RefundStarsCharge {
            user_id: self.resolve_user(&user_id.into()).await?,
            charge_id: telegram_payment_charge_id.into(),
        };
        self.invoke(&request).await.map(drop)
    }
}
