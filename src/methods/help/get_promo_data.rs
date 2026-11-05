use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PromoData;

impl Client {
    /// Returns the promoted chat or announcement Telegram wants shown. `None` if there is none.
    pub async fn get_promo_data(&self) -> Result<Option<PromoData>> {
        let base::help::PromoData::PromoData(promo) = self.invoke(&functions::help::GetPromoData {}).await? else {
            return Ok(None);
        };
        let mut promo = *promo;
        let entities = Entities::new(self, std::mem::take(&mut promo.users), std::mem::take(&mut promo.chats));
        Ok(Some(PromoData::parse(promo, &entities)))
    }
}
