use crate::raw::{base, types};

pub(crate) const CHANNEL_OFFSET: i64 = 1_000_000_000_000;

const BASE64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

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

pub(crate) fn channel_id_of(chat_id: i64) -> i64 {
    -CHANNEL_OFFSET - chat_id
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

pub(crate) fn chats_of(chats: base::messages::Chats) -> Vec<base::Chat> {
    match chats {
        base::messages::Chats::Chats(c) => c.chats,
        base::messages::Chats::ChatsSlice(c) => c.chats,
    }
}

pub(crate) fn input_sticker_set(name: &str) -> base::InputStickerSet {
    let name = types::InputStickerSetShortName { short_name: name.into() };
    base::InputStickerSet::InputStickerSetShortName(Box::new(name))
}

pub(crate) fn input_user(peer: base::InputPeer) -> Option<base::InputUser> {
    match peer {
        base::InputPeer::InputPeerUser(p) => {
            let user = types::InputUser { user_id: p.user_id, access_hash: p.access_hash };
            Some(base::InputUser::InputUser(Box::new(user)))
        }
        base::InputPeer::InputPeerUserFromMessage(p) => {
            let user = types::InputUserFromMessage { peer: p.peer, msg_id: p.msg_id, user_id: p.user_id };
            Some(base::InputUser::InputUserFromMessage(Box::new(user)))
        }
        base::InputPeer::InputPeerSelf => Some(base::InputUser::InputUserSelf),
        _ => None,
    }
}

/// Without padding, as session strings and Bot API inline message ids use it.
pub(crate) fn base64url_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(BASE64URL[(n >> (18 - 6 * i)) as usize & 63] as char);
        }
    }
    out
}

pub(crate) fn base64url_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes() {
        acc = (acc << 6) | BASE64URL.iter().position(|&a| a == c)? as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}
