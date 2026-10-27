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
    base64_encode_with(data, BASE64URL, false)
}

pub(crate) fn base64_encode(data: &[u8]) -> String {
    base64_encode_with(data, b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/", true)
}

fn base64_encode_with(data: &[u8], alphabet: &[u8; 64], pad: bool) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(alphabet[(n >> (18 - 6 * i)) as usize & 63] as char);
        }
        if pad {
            out.extend(std::iter::repeat_n('=', 3 - chunk.len()));
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

/// Takes both alphabets, padded or not: MTProxy links carry either.
pub(crate) fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.trim_end_matches('=').bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        };
        acc = (acc << 6 | v as u32) & 0xffff;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::{base64_decode, base64_encode, base64url_decode, base64url_encode};

    #[test]
    fn base64url_is_strict_and_proxy_secrets_are_not() {
        assert_eq!(base64url_encode(&[0xfb, 0xff]), "-_8");
        assert_eq!(base64url_decode("-_8").as_deref(), Some(&[0xfb, 0xff][..]));
        assert_eq!((base64url_decode("+_8"), base64url_decode("-_8=")), (None, None));
        assert_eq!(base64_encode(b"user:pw"), "dXNlcjpwdw==");
        assert_eq!(base64_decode("+/8=").as_deref(), Some(&[0xfb, 0xff][..]));
    }
}
