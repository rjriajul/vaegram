use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::ConnectedBot;

impl Client {
    pub async fn get_connected_bots(&self) -> Result<Vec<ConnectedBot>> {
        let base::account::ConnectedBots::ConnectedBots(b) =
            self.invoke(&functions::account::GetConnectedBots {}).await?;
        let b = *b;
        let entities = Entities::new(self, b.users, Vec::new());
        Ok(b.connected_bots.into_iter().map(|bot| ConnectedBot::parse(bot, &entities)).collect())
    }
}
