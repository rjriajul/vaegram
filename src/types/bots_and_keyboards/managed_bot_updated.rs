use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::types;
use crate::types::User;

/// Update about a bot that this bot manages.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ManagedBotUpdated {
    /// The user who created the managed bot.
    pub user: User,
    /// The managed bot.
    pub bot: User,
}

impl ManagedBotUpdated {
    pub(crate) fn parse(_client: &Client, raw: &types::UpdateManagedBot, entities: &Entities) -> ManagedBotUpdated {
        ManagedBotUpdated { user: User::parse(raw.user_id, entities), bot: User::parse(raw.bot_id, entities) }
    }

    /// Returns the managed bot's current token.
    pub async fn get_token(&self, client: &Client) -> Result<String> {
        client.get_managed_bot_token(self.bot.id).await
    }

    /// Revokes the managed bot's token and returns the new one.
    pub async fn replace_token(&self, client: &Client) -> Result<String> {
        client.replace_managed_bot_token(self.bot.id).await
    }
}
