use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::{PeerRef, ReactionType};

/// Request built by [`Client::set_chat_available_reactions`].
#[must_use = "a request does nothing until it is awaited"]
pub struct SetChatAvailableReactions<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    all: Option<bool>,
    reactions: Vec<ReactionType>,
    max_reactions: Option<i32>,
    paid_reactions: Option<bool>,
}

impl Client {
    /// Sets the reactions members can put on messages of a group or channel the account administers.
    ///
    /// Awaiting the request without calling [`all`](SetChatAvailableReactions::all) or
    /// [`reaction`](SetChatAvailableReactions::reaction) turns reactions off. Awaiting it returns `()`.
    pub fn set_chat_available_reactions(&self, chat_id: impl Into<PeerRef>) -> SetChatAvailableReactions<'_> {
        SetChatAvailableReactions {
            client: self,
            chat_id: chat_id.into(),
            all: None,
            reactions: Vec::new(),
            max_reactions: None,
            paid_reactions: None,
        }
    }
}

impl SetChatAvailableReactions<'_> {
    /// Allows every emoji reaction, and custom emoji too when `allow_custom_emoji` is set. Can't be combined with
    /// [`reaction`](SetChatAvailableReactions::reaction): awaiting then fails with `Error::InvalidArgument`.
    pub fn all(mut self, allow_custom_emoji: bool) -> Self {
        self.all = Some(allow_custom_emoji);
        self
    }

    /// Allows one more reaction. Call it again for more.
    pub fn reaction(mut self, reaction: impl Into<ReactionType>) -> Self {
        self.reactions.push(reaction.into());
        self
    }

    /// Limits how many different reactions one message can hold.
    pub fn max_reactions(mut self, max_reactions: i32) -> Self {
        self.max_reactions = Some(max_reactions);
        self
    }

    /// Allows or forbids paid reactions, which are bought with Telegram Stars.
    pub fn paid_reactions(mut self, enabled: bool) -> Self {
        self.paid_reactions = Some(enabled);
        self
    }

    async fn send(self) -> Result<()> {
        let available_reactions = match self.all {
            Some(_) if !self.reactions.is_empty() => {
                return Err(Error::InvalidArgument("all and reaction can't be combined"));
            }
            Some(allow_custom) => base::ChatReactions::ChatReactionsAll(types::ChatReactionsAll { allow_custom }),
            None if self.reactions.is_empty() => base::ChatReactions::ChatReactionsNone,
            None => {
                let reactions = self.reactions.iter().map(ReactionType::write).collect();
                base::ChatReactions::ChatReactionsSome(Box::new(types::ChatReactionsSome { reactions }))
            }
        };
        let peer = self.client.resolve_peer(self.chat_id).await?;
        let request = functions::messages::SetChatAvailableReactions {
            peer,
            available_reactions,
            reactions_limit: self.max_reactions,
            paid_enabled: self.paid_reactions,
        };
        self.client.invoke(&request).await?;
        Ok(())
    }
}

into_future!(SetChatAvailableReactions, ());

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::ReactionType;

    fn done() -> crate::Result<Vec<u8>> {
        let updates = types::Updates { updates: Vec::new(), users: Vec::new(), chats: Vec::new(), date: 1, seq: 0 };
        Ok(base::Updates::Updates(Box::new(updates)).to_bytes())
    }

    fn request(available_reactions: base::ChatReactions, limit: Option<i32>, paid: Option<bool>) -> Vec<u8> {
        let peer = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 });
        let request = functions::messages::SetChatAvailableReactions {
            peer,
            available_reactions,
            reactions_limit: limit,
            paid_enabled: paid,
        };
        request.to_bytes()
    }

    #[tokio::test]
    async fn sends_none_all_or_the_listed_reactions() {
        let (client, mut sent) = Client::scripted_raw(vec![done(), done(), done()]);
        client.set_chat_available_reactions(-5_i64).await.unwrap();
        client.set_chat_available_reactions(-5_i64).all(true).await.unwrap();
        let call = client.set_chat_available_reactions(-5_i64).reaction("👍").reaction(ReactionType::CustomEmoji(7));
        call.max_reactions(3).paid_reactions(true).await.unwrap();

        let none = base::ChatReactions::ChatReactionsNone;
        assert_eq!(sent.try_recv().unwrap(), request(none, None, None));
        let all = base::ChatReactions::ChatReactionsAll(types::ChatReactionsAll { allow_custom: true });
        assert_eq!(sent.try_recv().unwrap(), request(all, None, None));
        let like = base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: "👍".into() }));
        let custom = base::Reaction::ReactionCustomEmoji(types::ReactionCustomEmoji { document_id: 7 });
        let some = types::ChatReactionsSome { reactions: vec![like, custom] };
        let some = base::ChatReactions::ChatReactionsSome(Box::new(some));
        assert_eq!(sent.try_recv().unwrap(), request(some, Some(3), Some(true)));
    }

    #[tokio::test]
    async fn all_together_with_a_list_is_refused_without_a_request() {
        let (client, mut sent) = Client::scripted_raw(Vec::new());
        let result = client.set_chat_available_reactions(-5_i64).all(false).reaction("👍").await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        assert!(sent.try_recv().is_err());
    }
}
