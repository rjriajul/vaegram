use crate::raw::base;
use crate::types::ReactionType;
use crate::utils::get_peer_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerReaction {
    pub peer_id: i64,
    pub reaction: ReactionType,
    pub date: i32,
    pub big: bool,
    pub unread: bool,
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
