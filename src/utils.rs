use crate::raw::{base, types};

pub(crate) const CHANNEL_OFFSET: i64 = 1_000_000_000_000;

pub fn get_peer_id(peer: &base::Peer) -> i64 {
    match peer {
        base::Peer::PeerUser(p) => p.user_id,
        base::Peer::PeerChat(p) => -p.chat_id,
        base::Peer::PeerChannel(p) => get_channel_id(p.channel_id),
    }
}

pub fn get_channel_id(channel_id: i64) -> i64 {
    -CHANNEL_OFFSET - channel_id
}

pub fn get_peer(chat_id: i64) -> base::Peer {
    if chat_id > 0 {
        base::Peer::PeerUser(Box::new(types::PeerUser { user_id: chat_id }))
    } else if chat_id <= -CHANNEL_OFFSET {
        base::Peer::PeerChannel(Box::new(types::PeerChannel { channel_id: -chat_id - CHANNEL_OFFSET }))
    } else {
        base::Peer::PeerChat(Box::new(types::PeerChat { chat_id: -chat_id }))
    }
}
