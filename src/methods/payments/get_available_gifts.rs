use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::Gift;

impl Client {
    pub async fn get_available_gifts(&self) -> Result<Vec<Gift>> {
        let gifts = self.invoke(&functions::payments::GetStarGifts { hash: 0 }).await?;
        let base::payments::StarGifts::StarGifts(gifts) = gifts else {
            return Err(Error::Unexpected("payments.getStarGifts returned no gift list"));
        };
        let entities = Entities::new(self, gifts.users, gifts.chats);
        Ok(gifts
            .gifts
            .iter()
            .filter_map(|g| match g {
                base::StarGift::StarGift(g) => Gift::parse(g, &entities),
                base::StarGift::StarGiftUnique(_) => None,
            })
            .collect())
    }
}
