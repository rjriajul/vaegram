use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PremiumPromo;

impl Client {
    /// Returns Telegram's Premium status text and the subscription periods on offer.
    pub async fn get_premium_promo(&self) -> Result<PremiumPromo> {
        let base::help::PremiumPromo::PremiumPromo(promo) = self.invoke(&functions::help::GetPremiumPromo {}).await?;
        Ok(PremiumPromo::parse(*promo))
    }
}
