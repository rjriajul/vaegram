use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{PeerRef, ReactionsUpdated};

impl Client {
    /// Returns the current reactions of messages of one chat.
    ///
    /// Telegram answers only for messages that have reactions. An empty `message_ids` sends nothing.
    pub async fn get_messages_reactions(
        &self,
        chat_id: impl Into<PeerRef>,
        message_ids: &[i32],
    ) -> Result<Vec<ReactionsUpdated>> {
        if message_ids.is_empty() {
            return Ok(Vec::new());
        }
        let peer = self.resolve_peer(chat_id).await?;
        let request = functions::messages::GetMessagesReactions { peer, id: message_ids.to_vec() };
        let (updates, users, chats) = match self.invoke(&request).await? {
            base::Updates::Updates(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdatesCombined(u) => (u.updates, u.users, u.chats),
            base::Updates::UpdateShort(u) => (vec![u.update], Vec::new(), Vec::new()),
            _ => Default::default(),
        };
        let entities = Entities::new(self, users, chats);
        Ok(updates
            .iter()
            .filter_map(|update| match update {
                base::Update::UpdateMessageReactions(u) => Some(ReactionsUpdated::parse(self, u, &entities)),
                _ => None,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::ReactionType;
    use crate::utils::get_peer;

    fn update(msg_id: i32, emoticon: &str, count: i32) -> base::Update {
        let reaction = base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: emoticon.into() }));
        let count = types::ReactionCount { chosen_order: None, reaction, count };
        let reactions = types::MessageReactions {
            min: false,
            can_see_list: false,
            reactions_as_tags: false,
            results: vec![base::ReactionCount::ReactionCount(Box::new(count))],
            recent_reactions: None,
            top_reactors: None,
        };
        base::Update::UpdateMessageReactions(Box::new(types::UpdateMessageReactions {
            peer: get_peer(-5),
            msg_id,
            top_msg_id: None,
            saved_peer_id: None,
            reactions: base::MessageReactions::MessageReactions(Box::new(reactions)),
        }))
    }

    #[tokio::test]
    async fn reads_the_reactions_of_each_message() {
        let updates = types::Updates {
            updates: vec![update(3, "👍", 2), update(4, "🔥", 1)],
            users: Vec::new(),
            chats: Vec::new(),
            date: 1,
            seq: 0,
        };
        let (client, mut sent) = Client::scripted_raw(vec![Ok(base::Updates::Updates(Box::new(updates)).to_bytes())]);
        let found = client.get_messages_reactions(-5_i64, &[3, 4, 5]).await.unwrap();
        let counts: Vec<_> = found
            .iter()
            .map(|r| (r.chat.id, r.message_id, r.reactions.reactions.iter().map(|c| c.total_count).collect::<Vec<_>>()))
            .collect();
        assert_eq!(counts, [(-5, 3, vec![2]), (-5, 4, vec![1])]);
        assert_eq!(found[1].reactions.reactions[0].kind, ReactionType::Emoji("🔥".into()));
        let peer = base::InputPeer::InputPeerChat(types::InputPeerChat { chat_id: 5 });
        let expected = functions::messages::GetMessagesReactions { peer, id: vec![3, 4, 5] };
        assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
    }

    #[tokio::test]
    async fn no_ids_send_nothing() {
        let (client, mut sent) = Client::recording_raw();
        assert!(client.get_messages_reactions(-5_i64, &[]).await.unwrap().is_empty());
        assert!(sent.try_recv().is_err());
    }
}
