use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::User;
use crate::utils::get_peer_id;

/// The sender a channel's direct messages conversation belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DirectMessagesTopic {
    /// Bot API style id of the peer the topic is for.
    pub topic_id: i64,
    /// The sender. `None` when the peer isn't a user.
    pub user: Option<User>,
}

impl DirectMessagesTopic {
    pub(crate) fn parse(peer: &base::Peer, entities: &Entities) -> DirectMessagesTopic {
        let user = match peer {
            base::Peer::PeerUser(p) => Some(User::parse(p.user_id, entities)),
            _ => None,
        };
        DirectMessagesTopic { topic_id: get_peer_id(peer), user }
    }
}
