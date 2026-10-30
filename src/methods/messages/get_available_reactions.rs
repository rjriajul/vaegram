use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::AvailableReaction;

impl Client {
    pub async fn get_available_reactions(&self) -> Result<Vec<AvailableReaction>> {
        match self.invoke(&functions::messages::GetAvailableReactions { hash: 0 }).await? {
            base::messages::AvailableReactions::AvailableReactions(r) => {
                Ok(r.reactions.into_iter().map(AvailableReaction::parse).collect())
            }
            base::messages::AvailableReactions::AvailableReactionsNotModified => Ok(Vec::new()),
        }
    }
}
