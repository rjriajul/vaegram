use crate::raw::base;
use crate::types::ReactionType;
use crate::utils::get_peer_id;

/// A reaction one chat or user left on a message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerReaction {
    /// Id of the user or chat that reacted.
    pub peer_id: i64,
    /// The reaction they chose.
    pub reaction: ReactionType,
    /// Unix time the reaction was left.
    pub date: i32,
    /// The reaction was sent with the big animation.
    pub big: bool,
    /// The current account has not seen the reaction yet.
    pub unread: bool,
    /// The current account left the reaction.
    pub my: bool,
}

impl PeerReaction {
    pub(crate) fn parse(raw: &base::MessagePeerReaction) -> Option<PeerReaction> {
        let base::MessagePeerReaction::MessagePeerReaction(r) = raw;
        Some(PeerReaction {
            peer_id: get_peer_id(&r.peer_id),
            reaction: ReactionType::parse(&r.reaction)?,
            date: r.date,
            big: r.big,
            unread: r.unread,
            my: r.my,
        })
    }
}
