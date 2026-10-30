use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::ReactionType;

impl Client {
    pub async fn get_top_reactions(&self, limit: i32) -> Result<Vec<ReactionType>> {
        match self.invoke(&functions::messages::GetTopReactions { limit, hash: 0 }).await? {
            base::messages::Reactions::Reactions(r) => Ok(r.reactions.iter().filter_map(ReactionType::parse).collect()),
            base::messages::Reactions::ReactionsNotModified => Ok(Vec::new()),
        }
    }
}
