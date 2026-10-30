use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{PeerRef, ReactionType};

impl Client {
    pub async fn send_story_reaction(
        &self,
        chat_id: impl Into<PeerRef>,
        story_id: i32,
        reaction: Option<ReactionType>,
    ) -> Result<()> {
        let reaction = reaction.map_or(base::Reaction::ReactionEmpty, |r| r.write());
        let peer = self.resolve_peer(chat_id).await?;
        let request = functions::stories::SendReaction { add_to_recent: true, peer, story_id, reaction };
        self.invoke(&request).await?;
        Ok(())
    }
}
