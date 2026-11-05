use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::types;
use crate::types::User;

/// Update about a bot that this bot manages.
#[derive(Debug, Clone)]
pub struct ManagedBotUpdated {
    /// The user who created the managed bot.
    pub user: User,
    /// The managed bot.
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

    /// The client that received the update.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Returns the managed bot's current token.
    pub async fn get_token(&self) -> Result<String> {
        self.client.get_managed_bot_token(self.bot.id).await
    }

    /// Revokes the managed bot's token and returns the new one.
    pub async fn replace_token(&self) -> Result<String> {
        self.client.replace_managed_bot_token(self.bot.id).await
    }
}
