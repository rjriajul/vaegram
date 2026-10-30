use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::SavedReactionTag;

impl Client {
    pub async fn get_saved_reaction_tags(&self) -> Result<Vec<SavedReactionTag>> {
        match self.invoke(&functions::messages::GetSavedReactionTags { peer: None, hash: 0 }).await? {
            base::messages::SavedReactionTags::SavedReactionTags(t) => {
                Ok(t.tags.into_iter().filter_map(SavedReactionTag::parse).collect())
            }
            base::messages::SavedReactionTags::SavedReactionTagsNotModified => Ok(Vec::new()),
        }
    }
}
