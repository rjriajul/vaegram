use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{PeerRef, ReactionType};

/// Request built by [`Client::send_story_reaction`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SendStoryReaction<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    story_id: i32,
    reaction: Option<ReactionType>,
}

impl Client {
    /// Sets the current account's reaction on a story.
    ///
    /// Awaiting the request without a [`reaction`](SendStoryReaction::reaction) removes the existing one.
    pub fn send_story_reaction(&self, chat_id: impl Into<PeerRef>, story_id: i32) -> SendStoryReaction<'_> {
        SendStoryReaction { client: self, chat_id: chat_id.into(), story_id, reaction: None }
    }
}

impl SendStoryReaction<'_> {
    /// Reacts with this reaction. A story takes one, so a later call replaces it.
    pub fn reaction(mut self, reaction: impl Into<ReactionType>) -> Self {
        self.reaction = Some(reaction.into());
        self
    }

    async fn send(self) -> Result<()> {
        let reaction = self.reaction.map_or(base::Reaction::ReactionEmpty, |r| r.write());
        let peer = self.client.resolve_peer(self.chat_id).await?;
        let request = functions::stories::SendReaction { add_to_recent: true, peer, story_id: self.story_id, reaction };
        self.client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(SendStoryReaction, ());
