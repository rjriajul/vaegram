use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::functions;
use crate::types::{PeerRef, ReactionType};

/// Request built by [`Client::send_reaction`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SendReaction<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    reactions: Vec<ReactionType>,
    big: bool,
    add_to_recent: bool,
    // Why awaiting fails before anything is sent, when the message can't take a reaction at all.
    refused: Option<&'static str>,
}

impl Client {
    /// Sets the current account's reactions on a message.
    ///
    /// Awaiting the request without adding any reaction removes the existing ones.
    pub fn send_reaction(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> SendReaction<'_> {
        SendReaction {
            client: self,
            chat_id: chat_id.into(),
            message_id,
            reactions: Vec::new(),
            big: false,
            add_to_recent: true,
            refused: None,
        }
    }
}

impl SendReaction<'_> {
    /// Adds a reaction. Call it again for more.
    pub fn reaction(mut self, reaction: impl Into<ReactionType>) -> Self {
        self.reactions.push(reaction.into());
        self
    }

    /// Plays the big reaction animation.
    pub fn big(mut self) -> Self {
        self.big = true;
        self
    }

    /// Whether Telegram adds the reaction to the account's recently used ones. The default is `true`.
    pub fn add_to_recent(mut self, add_to_recent: bool) -> Self {
        self.add_to_recent = add_to_recent;
        self
    }

    pub(crate) fn refuse(mut self, reason: &'static str) -> Self {
        self.refused = Some(reason);
        self
    }

    async fn send(self) -> Result<()> {
        if let Some(reason) = self.refused {
            return Err(Error::InvalidArgument(reason));
        }
        let (chat_id, peer) = self.client.resolve(&self.chat_id).await?;
        let reaction = (!self.reactions.is_empty()).then(|| self.reactions.iter().map(ReactionType::write).collect());
        let request = functions::messages::SendReaction {
            big: self.big,
            add_to_recent: self.add_to_recent,
            peer,
            msg_id: self.message_id,
            reaction,
        };
        self.client.invoke(&request).await?;
        // The answer is a quiet update, so the cached copy would keep its old reactions.
        self.client.0.caches.evict(chat_id, &[self.message_id]);
        Ok(())
    }
}

into_future!(SendReaction, ());

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};

    #[tokio::test]
    async fn add_to_recent_defaults_to_true_and_can_be_turned_off() {
        let (client, mut sent) = Client::recording_raw();
        let peer = types::InputPeerUser { user_id: 7, access_hash: 2 };
        client.0.peers.lock().unwrap().insert(7, base::InputPeer::InputPeerUser(Box::new(peer)));
        let request = |add_to_recent| {
            let peer = types::InputPeerUser { user_id: 7, access_hash: 2 };
            let peer = base::InputPeer::InputPeerUser(Box::new(peer));
            functions::messages::SendReaction { big: false, add_to_recent, peer, msg_id: 4, reaction: None }.to_bytes()
        };

        let _ = client.send_reaction(7, 4).await;
        assert_eq!(sent.recv().await.unwrap(), request(true));
        let _ = client.send_reaction(7, 4).add_to_recent(false).await;
        assert_eq!(sent.recv().await.unwrap(), request(false));
    }
}
