use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::PeerRef;

impl Client {
    pub async fn edit_user_star_subscription(
        &self,
        user_id: impl Into<PeerRef>,
        telegram_payment_charge_id: &str,
        is_canceled: bool,
    ) -> Result<bool> {
        let request = functions::payments::BotCancelStarsSubscription {
            restore: !is_canceled,
            user_id: self.resolve_user(&user_id.into()).await?,
            charge_id: telegram_payment_charge_id.into(),
        };
        self.invoke(&request).await
    }
}
