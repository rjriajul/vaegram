use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::MyBoost;

impl Client {
    pub async fn get_my_boosts(&self) -> Result<Vec<MyBoost>> {
        let boosts = self.invoke(&functions::premium::GetMyBoosts {}).await?;
        Ok(my_boosts(self, boosts))
    }
}

pub(super) fn my_boosts(client: &Client, raw: base::premium::MyBoosts) -> Vec<MyBoost> {
    let base::premium::MyBoosts::MyBoosts(raw) = raw;
    let entities = Entities::new(client, raw.users, raw.chats);
    raw.my_boosts.iter().map(|base::MyBoost::MyBoost(b)| MyBoost::parse(b, &entities)).collect()
}
