use crate::raw::base;

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
