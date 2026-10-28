use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::types;
use crate::types::User;

#[derive(Debug, Clone)]
pub struct ManagedBotUpdated {
    pub user: User,
    pub bot: User,
    client: Client,
}

impl ManagedBotUpdated {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateManagedBot, entities: &Entities) -> ManagedBotUpdated {
        ManagedBotUpdated {
            user: User::parse(raw.user_id, entities),
            bot: User::parse(raw.bot_id, entities),
            client: client.clone(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub async fn get_token(&self) -> Result<String> {
        self.client.get_managed_bot_token(self.bot.id).await
    }

    pub async fn replace_token(&self) -> Result<String> {
        self.client.replace_managed_bot_token(self.bot.id).await
    }
}
