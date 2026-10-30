use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::ReactionType;

impl Client {
    pub async fn set_default_reaction(&self, reaction: impl Into<ReactionType>) -> Result<()> {
        self.invoke(&functions::messages::SetDefaultReaction { reaction: reaction.into().write() }).await?;
        Ok(())
    }
}
