use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::{base, types};
use crate::types::{Chat, ReactionType, User};

#[derive(Clone)]
pub struct MessageReactionUpdated {
    pub chat: Chat,
    pub message_id: i32,
    pub user: Option<User>,
    /// Set instead of `user` when the reaction was made on behalf of a chat.
    pub actor_chat: Option<Chat>,
    pub date: i32,
    pub old_reaction: Vec<ReactionType>,
    pub new_reaction: Vec<ReactionType>,
    client: Client,
}

impl MessageReactionUpdated {
    pub(crate) fn parse(
        client: &Client,
        raw: &types::UpdateBotMessageReaction,
        entities: &Entities,
    ) -> MessageReactionUpdated {
        let (user, actor_chat) = match &raw.actor {
            base::Peer::PeerUser(p) => (Some(User::parse(p.user_id, entities)), None),
            peer => (None, Some(Chat::parse(peer, entities, false))),
        };
        MessageReactionUpdated {
            chat: Chat::parse(&raw.peer, entities, false),
            message_id: raw.msg_id,
            user,
            actor_chat,
            date: raw.date,
            old_reaction: raw.old_reactions.iter().filter_map(ReactionType::parse).collect(),
            new_reaction: raw.new_reactions.iter().filter_map(ReactionType::parse).collect(),
            client: client.clone(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

impl fmt::Debug for MessageReactionUpdated {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MessageReactionUpdated")
            .field("chat", &self.chat)
            .field("message_id", &self.message_id)
            .field("user", &self.user)
            .field("actor_chat", &self.actor_chat)
            .field("date", &self.date)
            .field("old_reaction", &self.old_reaction)
            .field("new_reaction", &self.new_reaction)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::MessageReactionUpdated;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::types::ReactionType;
    use crate::utils::{get_channel_id, get_peer};

    fn update(actor: i64, new_reactions: Vec<base::Reaction>) -> types::UpdateBotMessageReaction {
        types::UpdateBotMessageReaction {
            peer: get_peer(get_channel_id(42)),
            msg_id: 10,
            date: 1_700_000_000,
            actor: get_peer(actor),
            old_reactions: vec![base::Reaction::ReactionEmpty],
            new_reactions,
            qts: 1,
        }
    }

    #[test]
    fn splits_actor_and_drops_empty_reactions() {
        let client = Client::offline();
        let entities = Entities::default();
        let thumbs = base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: "👍".into() }));
        let custom = base::Reaction::ReactionCustomEmoji(Box::new(types::ReactionCustomEmoji { document_id: 5 }));

        let r = MessageReactionUpdated::parse(&client, &update(7, vec![thumbs, custom]), &entities);
        assert_eq!((r.chat.id, r.user.map(|u| u.id), r.actor_chat.map(|c| c.id)), (get_channel_id(42), Some(7), None));
        assert_eq!(
            (r.old_reaction, r.new_reaction),
            (vec![], vec![ReactionType::Emoji("👍".into()), ReactionType::CustomEmoji(5)])
        );

        let r = MessageReactionUpdated::parse(&client, &update(get_channel_id(42), vec![]), &entities);
        assert_eq!((r.user, r.actor_chat.map(|c| c.id)), (None, Some(get_channel_id(42))));
    }
}
