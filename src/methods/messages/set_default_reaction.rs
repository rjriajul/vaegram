use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::ReactionType;

impl Client {
    /// Sets the reaction the current account uses.
    pub async fn set_default_reaction(&self, reaction: impl Into<ReactionType>) -> Result<()> {
        self.invoke(&functions::messages::SetDefaultReaction { reaction: reaction.into().write() }).await?;
        Ok(())
    }
}
