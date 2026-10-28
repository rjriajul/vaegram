use crate::dispatcher::Entities;
use crate::raw::base;
use crate::types::User;
use crate::utils::get_peer_id;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectMessagesTopic {
    pub topic_id: i64,
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
