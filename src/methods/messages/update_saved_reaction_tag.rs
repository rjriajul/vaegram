use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::ReactionType;

impl Client {
    pub async fn update_saved_reaction_tag(
        &self,
        reaction: impl Into<ReactionType>,
        title: impl Into<String>,
    ) -> Result<()> {
        let title = Some(title.into()).filter(|t| !t.is_empty());
        self.invoke(&functions::messages::UpdateSavedReactionTag { reaction: reaction.into().write(), title }).await?;
        Ok(())
    }
}
