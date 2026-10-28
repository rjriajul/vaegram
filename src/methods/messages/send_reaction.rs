use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{PeerRef, ReactionType};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendReaction<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    reactions: Vec<ReactionType>,
    big: bool,
}

impl Client {
    /// Sets the reactions of the current account on a message; awaiting it without any reaction removes them.
    pub fn send_reaction(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> SendReaction<'_> {
        SendReaction { client: self, chat_id: chat_id.into(), message_id, reactions: Vec::new(), big: false }
    }
}

impl SendReaction<'_> {
    pub fn reaction(mut self, reaction: impl Into<ReactionType>) -> Self {
        self.reactions.push(reaction.into());
        self
    }

    pub fn big(mut self) -> Self {
        self.big = true;
        self
    }

    async fn send(self) -> Result<()> {
        let peer = self.client.resolve_peer(self.chat_id).await?;
        let reaction = (!self.reactions.is_empty()).then(|| self.reactions.iter().map(ReactionType::write).collect());
        let request = functions::messages::SendReaction {
            big: self.big,
            add_to_recent: false,
            peer,
            msg_id: self.message_id,
            reaction,
        };
        self.client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(SendReaction, ());
