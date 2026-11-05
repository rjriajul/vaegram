use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::{PaidReactor, ReactionCount};

/// The reactions on a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MessageReactions {
    /// Each reaction on the message with its count.
    pub reactions: Vec<ReactionCount>,
    /// The reactions are tags on a message in Saved Messages.
    pub are_tags: bool,
    /// The top senders of paid reactions.
    pub paid_reactors: Vec<PaidReactor>,
    /// The list of who reacted with what can be fetched.
    pub can_get_added_reactions: bool,
}

impl MessageReactions {
    pub(crate) fn parse(raw: &base::MessageReactions, entities: &Entities) -> MessageReactions {
        let base::MessageReactions::MessageReactions(r) = raw;
        MessageReactions {
            reactions: r.results.iter().filter_map(ReactionCount::parse).collect(),
            are_tags: r.reactions_as_tags,
            paid_reactors: r.top_reactors.iter().flatten().map(|p| PaidReactor::parse(p, entities)).collect(),
            can_get_added_reactions: r.can_see_list,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MessageReactions;
    use crate::dispatcher::Entities;
    use crate::raw::{base, types};
    use crate::types::ReactionType;

    fn count(emoticon: &str, count: i32) -> base::ReactionCount {
        let reaction = base::Reaction::ReactionEmoji(Box::new(types::ReactionEmoji { emoticon: emoticon.into() }));
        base::ReactionCount::ReactionCount(Box::new(types::ReactionCount { chosen_order: None, reaction, count }))
    }

    fn reactions(raw: types::MessageReactions) -> MessageReactions {
        MessageReactions::parse(&base::MessageReactions::MessageReactions(Box::new(raw)), &Entities::default())
    }

    #[test]
    fn reads_a_min_block_of_tags() {
        let raw = types::MessageReactions {
            min: true,
            can_see_list: false,
            reactions_as_tags: true,
            results: vec![count("👍", 2)],
            recent_reactions: None,
            top_reactors: None,
        };
        let parsed = reactions(raw);
        assert!(parsed.are_tags && !parsed.can_get_added_reactions && parsed.paid_reactors.is_empty());
        let counts: Vec<_> = parsed.reactions.iter().map(|r| (r.kind.clone(), r.total_count)).collect();
        assert_eq!(counts, [(ReactionType::Emoji("👍".into()), 2)]);
    }

    #[test]
    fn reads_top_paid_reactors() {
        let reactor = types::MessageReactor { top: true, my: true, anonymous: false, peer_id: None, count: 5 };
        let raw = types::MessageReactions {
            min: false,
            can_see_list: true,
            reactions_as_tags: false,
            results: Vec::new(),
            recent_reactions: None,
            top_reactors: Some(vec![base::MessageReactor::MessageReactor(Box::new(reactor))]),
        };
        let parsed = reactions(raw);
        assert!(parsed.can_get_added_reactions && !parsed.are_tags);
        assert_eq!(parsed.paid_reactors.iter().map(|p| (p.star_count, p.is_me)).collect::<Vec<_>>(), [(5, true)]);
    }
}
