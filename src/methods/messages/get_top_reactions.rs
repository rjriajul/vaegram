use crate::Client;
use crate::errors::Result;
use crate::methods::pages::wire_limit;
use crate::raw::{base, functions};
use crate::types::ReactionType;

impl Client {
    /// Returns the reactions Telegram lists as top reactions, up to `limit`.
    pub async fn get_top_reactions(&self, limit: usize) -> Result<Vec<ReactionType>> {
        match self.invoke(&functions::messages::GetTopReactions { limit: wire_limit(limit), hash: 0 }).await? {
            base::messages::Reactions::Reactions(r) => Ok(r.reactions.iter().filter_map(ReactionType::parse).collect()),
            base::messages::Reactions::ReactionsNotModified => Ok(Vec::new()),
        }
    }
}
