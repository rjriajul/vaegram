use std::fmt::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::crypto::password::Srp;
use crate::crypto::{hmac_sha256, random};
use crate::errors::{Error, Result};
use crate::raw::{base, types};

pub(crate) const CHANNEL_OFFSET: i64 = 1_000_000_000_000;

const BASE64URL: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

/// The Bot-API-style id of a TL peer: users as they are, basic groups negated, channels as `-100…`
/// ([`get_channel_id`]). An id from the server out of range wraps instead of panicking.
pub fn get_peer_id(peer: &base::Peer) -> i64 {
    match peer {
        base::Peer::PeerUser(p) => p.user_id,
        base::Peer::PeerChat(p) => p.chat_id.wrapping_neg(),
        base::Peer::PeerChannel(p) => get_channel_id(p.channel_id),
    }
}

pub(crate) fn get_chat_id(chat: &base::Chat) -> Option<i64> {
    match chat {
        base::Chat::ChatEmpty(c) => Some(-c.id),
        base::Chat::Chat(c) => Some(-c.id),
        base::Chat::ChatForbidden(c) => Some(-c.id),
        base::Chat::Channel(c) => Some(get_channel_id(c.id)),
        base::Chat::ChannelForbidden(c) => Some(get_channel_id(c.id)),
        base::Chat::Community(_) | base::Chat::CommunityForbidden(_) => None,
    }
}

/// The Bot-API-style id of a channel or supergroup: `-1000000000000 - channel_id`, so 1234 is `-1000000001234`.
pub fn get_channel_id(channel_id: i64) -> i64 {
    (-CHANNEL_OFFSET).wrapping_sub(channel_id)
}

pub(crate) fn channel_id_of(chat_id: i64) -> i64 {
    -CHANNEL_OFFSET - chat_id
}

/// The TL peer of a Bot-API-style id: positive is a user, `-100…` (at most `-1000000000000`) a channel, and any
/// other id, 0 included, a basic group.
pub fn get_peer(chat_id: i64) -> base::Peer {
    if chat_id > 0 {
        base::Peer::PeerUser(types::PeerUser { user_id: chat_id })
    } else if chat_id <= -CHANNEL_OFFSET {
        base::Peer::PeerChannel(types::PeerChannel { channel_id: -(chat_id + CHANNEL_OFFSET) })
    } else {
        base::Peer::PeerChat(types::PeerChat { chat_id: -chat_id })
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

// The JPEG every stripped thumbnail shares, minus its scan data: only the size bytes at 164 and 166 change.
const STRIPPED_JPEG_HEADER: &[u8; 623] = b"\
    \xff\xd8\xff\xe0\x00\x10\x4a\x46\x49\x46\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00\xff\xdb\x00\x43\x00\x28\x1c\
    \x1e\x23\x1e\x19\x28\x23\x21\x23\x2d\x2b\x28\x30\x3c\x64\x41\x3c\x37\x37\x3c\x7b\x58\x5d\x49\x64\x91\x80\x99\
    \x96\x8f\x80\x8c\x8a\xa0\xb4\xe6\xc3\xa0\xaa\xda\xad\x8a\x8c\xc8\xff\xcb\xda\xee\xf5\xff\xff\xff\x9b\xc1\xff\
    \xff\xff\xfa\xff\xe6\xfd\xff\xf8\xff\xdb\x00\x43\x01\x2b\x2d\x2d\x3c\x35\x3c\x76\x41\x41\x76\xf8\xa5\x8c\xa5\
    \xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\
    \xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xf8\xff\xc0\x00\x11\
    \x08\x00\x00\x00\x00\x03\x01\x22\x00\x02\x11\x01\x03\x11\x01\xff\xc4\x00\x1f\x00\x00\x01\x05\x01\x01\x01\x01\
    \x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\xff\xc4\x00\xb5\x10\x00\
    \x02\x01\x03\x03\x02\x04\x03\x05\x05\x04\x04\x00\x00\x01\x7d\x01\x02\x03\x00\x04\x11\x05\x12\x21\x31\x41\x06\
    \x13\x51\x61\x07\x22\x71\x14\x32\x81\x91\xa1\x08\x23\x42\xb1\xc1\x15\x52\xd1\xf0\x24\x33\x62\x72\x82\x09\x0a\
    \x16\x17\x18\x19\x1a\x25\x26\x27\x28\x29\x2a\x34\x35\x36\x37\x38\x39\x3a\x43\x44\x45\x46\x47\x48\x49\x4a\x53\
    \x54\x55\x56\x57\x58\x59\x5a\x63\x64\x65\x66\x67\x68\x69\x6a\x73\x74\x75\x76\x77\x78\x79\x7a\x83\x84\x85\x86\
    \x87\x88\x89\x8a\x92\x93\x94\x95\x96\x97\x98\x99\x9a\xa2\xa3\xa4\xa5\xa6\xa7\xa8\xa9\xaa\xb2\xb3\xb4\xb5\xb6\
    \xb7\xb8\xb9\xba\xc2\xc3\xc4\xc5\xc6\xc7\xc8\xc9\xca\xd2\xd3\xd4\xd5\xd6\xd7\xd8\xd9\xda\xe1\xe2\xe3\xe4\xe5\
    \xe6\xe7\xe8\xe9\xea\xf1\xf2\xf3\xf4\xf5\xf6\xf7\xf8\xf9\xfa\xff\xc4\x00\x1f\x01\x00\x03\x01\x01\x01\x01\x01\
    \x01\x01\x01\x01\x00\x00\x00\x00\x00\x00\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\xff\xc4\x00\xb5\x11\x00\
    \x02\x01\x02\x04\x04\x03\x04\x07\x05\x04\x04\x00\x01\x02\x77\x00\x01\x02\x03\x11\x04\x05\x21\x31\x06\x12\x41\
    \x51\x07\x61\x71\x13\x22\x32\x81\x08\x14\x42\x91\xa1\xb1\xc1\x09\x23\x33\x52\xf0\x15\x62\x72\xd1\x0a\x16\x24\
    \x34\xe1\x25\xf1\x17\x18\x19\x1a\x26\x27\x28\x29\x2a\x35\x36\x37\x38\x39\x3a\x43\x44\x45\x46\x47\x48\x49\x4a\
    \x53\x54\x55\x56\x57\x58\x59\x5a\x63\x64\x65\x66\x67\x68\x69\x6a\x73\x74\x75\x76\x77\x78\x79\x7a\x82\x83\x84\
    \x85\x86\x87\x88\x89\x8a\x92\x93\x94\x95\x96\x97\x98\x99\x9a\xa2\xa3\xa4\xa5\xa6\xa7\xa8\xa9\xaa\xb2\xb3\xb4\
    \xb5\xb6\xb7\xb8\xb9\xba\xc2\xc3\xc4\xc5\xc6\xc7\xc8\xc9\xca\xd2\xd3\xd4\xd5\xd6\xd7\xd8\xd9\xda\xe2\xe3\xe4\
    \xe5\xe6\xe7\xe8\xe9\xea\xf2\xf3\xf4\xf5\xf6\xf7\xf8\xf9\xfa\xff\xda\x00\x0c\x03\x01\x00\x02\x11\x03\x11\x00\
    \x3f\x00";

/// Turns a stripped thumbnail (the `bytes` of a `photoStrippedSize`, as chat photos and media carry) back into a
/// JPEG. `None` when the payload is too short or isn't a stripped thumbnail.
pub fn expand_inline_bytes(stripped: &[u8]) -> Option<Vec<u8>> {
    let [1, height, width, scan @ ..] = stripped else { return None };
    let mut jpeg = Vec::with_capacity(STRIPPED_JPEG_HEADER.len() + scan.len() + 2);
    jpeg.extend_from_slice(STRIPPED_JPEG_HEADER);
    jpeg[164] = *height;
    jpeg[166] = *width;
    jpeg.extend_from_slice(scan);
    jpeg.extend_from_slice(b"\xff\xd9");
    Some(jpeg)
}

/// Decodes a vector thumbnail (the `bytes` of a `photoPathSize`) into SVG path data, ready for the `d` attribute of
/// a `<path>` in a `viewBox="0 0 512 512"`.
pub fn decode_svg_path(encoded: &[u8]) -> String {
    const COMMANDS: &[u8; 64] = b"AACAAAAHAAALMAAAQASTAVAAAZaacaaaahaaalmaaaqastava.az0123456789-,";
    let mut path = String::with_capacity(encoded.len() * 3 + 2);
    path.push('M');
    for &byte in encoded {
        if byte >= 192 {
            path.push(char::from(COMMANDS[usize::from(byte - 192)]));
            continue;
        }
        if byte >= 128 {
            path.push(',');
        } else if byte >= 64 {
            path.push('-');
        }
        let _ = write!(path, "{}", byte & 63);
    }
    path.push('z');
    path
}

/// Validates the `initData` a Mini App received (`Telegram.WebApp.initData`) with the bot's token, as
/// <https://core.telegram.org/bots/webapps#validating-data-received-via-the-mini-app> describes, and returns its
/// fields in the order they came, `hash` included. Keys and values are decoded the way a browser's
/// `URLSearchParams` decodes them: `%XX` escapes, and `+` as a space.
///
/// With `max_age`, `auth_date` must also be at most that old by this machine's clock; a date ahead of the clock
/// passes. Only the hash made with the bot token is checked, not the Ed25519 `signature` meant for third parties.
///
/// Fails with [`Error::Security`] when there is no single `hash` of 64 hex digits, when it doesn't match, or when
/// `max_age` is given and `auth_date` is missing, not a number or too old; with [`Error::InvalidArgument`] when
/// `init_data` has a broken `%` escape or decodes to invalid UTF-8.
pub fn validate_web_app_data(
    bot_token: &str,
    init_data: &str,
    max_age: Option<Duration>,
) -> Result<Vec<(String, String)>> {
    let fields = init_data
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            Ok((form_decode(key)?, form_decode(value)?))
        })
        .collect::<Result<Vec<_>>>()?;
    let no_hash = Error::Security("Mini App data needs one hash of 64 hex digits");
    let mut hash = None;
    let mut checked = Vec::with_capacity(fields.len());
    for (key, value) in &fields {
        if key != "hash" {
            checked.push((key.as_str(), value.as_str()));
        } else if hash.replace(value).is_some() {
            return Err(no_hash);
        }
    }
    let Some(hash) = hash.and_then(|h| hex_32(h)) else { return Err(no_hash) };
    checked.sort_by(|a, b| a.0.cmp(b.0));
    let mut data_check = String::with_capacity(init_data.len());
    for (i, (key, value)) in checked.into_iter().enumerate() {
        if i > 0 {
            data_check.push('\n');
        }
        let _ = write!(data_check, "{key}={value}");
    }
    let secret = hmac_sha256(b"WebAppData", &[bot_token.as_bytes()]);
    let expected = hmac_sha256(&secret, &[data_check.as_bytes()]);
    // Constant time, so the comparison doesn't tell how many leading bytes of a forged hash are right.
    let diff = expected.iter().zip(&hash).fold(0, |acc, (a, b)| acc | (a ^ b));
    if std::hint::black_box(diff) != 0 {
        return Err(Error::Security("Mini App data doesn't match its hash"));
    }
    if let Some(max_age) = max_age {
        let auth_date =
            fields.iter().find(|(key, _)| key == "auth_date").and_then(|(_, date)| date.parse::<u64>().ok());
        let auth_date = auth_date.ok_or(Error::Security("Mini App data has no auth_date"))?;
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |now| now.as_secs());
        if Duration::from_secs(now.saturating_sub(auth_date)) > max_age {
            return Err(Error::Security("Mini App data is older than max_age"));
        }
    }
    Ok(fields)
}

/// Decodes one key or value of a query string: `%XX` escapes, and `+` as a space.
fn form_decode(s: &str) -> Result<String> {
    let broken = || Error::InvalidArgument("Mini App data isn't a valid query string");
    let mut out = Vec::with_capacity(s.len());
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        out.push(match b {
            b'+' => b' ',
            b'%' => {
                let (hi, lo) = (bytes.next().and_then(hex_digit), bytes.next().and_then(hex_digit));
                hi.zip(lo).map(|(hi, lo)| (hi << 4) | lo).ok_or_else(broken)?
            }
            b => b,
        });
    }
    String::from_utf8(out).map_err(|_| broken())
}

fn hex_digit(b: u8) -> Option<u8> {
    char::from(b).to_digit(16).map(|d| d as u8)
}

/// 64 hex digits, in either case, as 32 bytes.
fn hex_32(s: &str) -> Option<[u8; 32]> {
    let s = s.as_bytes();
    if s.len() != 64 {
        return None;
    }
    let mut out = [0; 32];
    for (byte, pair) in out.iter_mut().zip(s.chunks_exact(2)) {
        *byte = (hex_digit(pair[0])? << 4) | hex_digit(pair[1])?;
    }
    Some(out)
}

pub(crate) fn known_algo(
    algo: &base::PasswordKdfAlgo,
) -> Result<&types::PasswordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow> {
    match algo {
        base::PasswordKdfAlgo::PasswordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow(algo) => Ok(algo),
        _ => Err(Error::Unexpected("unknown password algorithm; update vaegram")),
    }
}

pub(crate) fn compute_password_check(
    p: &types::account::Password,
    password: &str,
) -> Result<base::InputCheckPasswordSRP> {
    let (Some(algo), Some(g_b), Some(srp_id)) = (&p.current_algo, &p.srp_b, p.srp_id) else {
        return Err(Error::InvalidArgument("this account has no password"));
    };
    let algo = known_algo(algo)?;
    let srp = Srp { salt1: &algo.salt1, salt2: &algo.salt2, g: algo.g, p: &algo.p, g_b };
    let (a, m1) = srp.check(password.as_bytes(), &random()).map_err(Error::Security)?;
    let check = types::InputCheckPasswordSRP { srp_id, a: a.to_vec(), m1: m1.to_vec() };
    Ok(base::InputCheckPasswordSRP::InputCheckPasswordSRP(Box::new(check)))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{
        base64_decode, base64_encode, base64url_decode, base64url_encode, decode_svg_path, expand_inline_bytes,
        form_decode, validate_web_app_data,
    };
    use crate::Error;

    const BOT_TOKEN: &str = "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11";
    // Signed with an independent HMAC-SHA256 implementation; `signature` is only part of the checked string here.
    const INIT_DATA: &str = "query_id=AAHdF6IQAAAAAN0XohDhrOrc&user=%7B%22id%22%3A279058397%2C%22first_name%22%3A%22Vlad\
        %22%2C%22username%22%3A%22vdkfrost%22%2C%22language_code%22%3A%22ru%22%7D&auth_date=1662771648\
        &signature=abc%2Bdef%2Fghi&hash=e3321ee2178b3e170a45df19e51faebff0901dfe274900304106e0bba5f8f9dd";

    #[test]
    fn web_app_data_passes_with_the_right_token() {
        let fields = validate_web_app_data(BOT_TOKEN, INIT_DATA, None).unwrap();
        let keys: Vec<&str> = fields.iter().map(|(key, _)| key.as_str()).collect();
        assert_eq!(keys, ["query_id", "user", "auth_date", "signature", "hash"]);
        assert_eq!(fields[1].1, r#"{"id":279058397,"first_name":"Vlad","username":"vdkfrost","language_code":"ru"}"#);
        assert_eq!(fields[3].1, "abc+def/ghi");
        let upper = INIT_DATA.replace("e3321ee2", "E3321EE2");
        assert!(validate_web_app_data(BOT_TOKEN, &upper, None).is_ok());
        // The order of the fields doesn't matter, only their sorted check string.
        let (head, hash) = INIT_DATA.split_once("&hash=").unwrap();
        assert!(validate_web_app_data(BOT_TOKEN, &format!("hash={hash}&{head}&"), None).is_ok());
    }

    #[test]
    fn web_app_data_refuses_tampering() {
        let security = |token, data: &str| matches!(validate_web_app_data(token, data, None), Err(Error::Security(_)));
        assert!(security("123456:other", INIT_DATA));
        assert!(security(BOT_TOKEN, &INIT_DATA.replace("1662771648", "1662771649")));
        assert!(security(BOT_TOKEN, &INIT_DATA.replace("Vlad", "Vlae")));
        assert!(security(BOT_TOKEN, &INIT_DATA.replace("e3321ee2", "e3321ee3")));
        assert!(security(BOT_TOKEN, &INIT_DATA.replace("query_id=", "query_iD=")));
        assert!(security(BOT_TOKEN, &format!("{INIT_DATA}&extra=1")));
        assert!(security(BOT_TOKEN, &format!("{INIT_DATA}&hash=00")));
        assert!(security(BOT_TOKEN, INIT_DATA.split_once("&hash=").unwrap().0));
        assert!(security(BOT_TOKEN, &INIT_DATA.replace("bba5f8f9dd", "bba5f8f9d")));
        assert!(security(BOT_TOKEN, &INIT_DATA.replace("bba5f8f9dd", "bba5f8f9dg")));
        let broken = format!("{INIT_DATA}&x=%E");
        assert!(matches!(validate_web_app_data(BOT_TOKEN, &broken, None), Err(Error::InvalidArgument(_))));
    }

    #[test]
    fn web_app_data_max_age_reads_auth_date() {
        let years = Duration::from_secs(100 * 365 * 24 * 3600);
        assert!(validate_web_app_data(BOT_TOKEN, INIT_DATA, Some(years)).is_ok());
        let old = validate_web_app_data(BOT_TOKEN, INIT_DATA, Some(Duration::from_secs(3600)));
        assert!(matches!(old, Err(Error::Security("Mini App data is older than max_age"))), "{old:?}");
    }

    #[test]
    fn form_decoding_follows_url_search_params() {
        assert_eq!(form_decode("a+b%20c%2B%e2%9C%93").unwrap(), "a b c+✓");
        for broken in ["%", "%4", "%zz", "%ff"] {
            assert!(matches!(form_decode(broken), Err(Error::InvalidArgument(_))), "{broken}");
        }
    }

    #[test]
    fn base64url_is_strict_and_proxy_secrets_are_not() {
        assert_eq!(base64url_encode(&[0xfb, 0xff]), "-_8");
        assert_eq!(base64url_decode("-_8").as_deref(), Some(&[0xfb, 0xff][..]));
        assert_eq!((base64url_decode("+_8"), base64url_decode("-_8=")), (None, None));
        assert_eq!(base64_encode(b"user:pw"), "dXNlcjpwdw==");
        assert_eq!(base64_decode("+/8=").as_deref(), Some(&[0xfb, 0xff][..]));
    }

    const STRIPPED: [u8; 28] = [
        0x01, 0x0e, 0x28, 0xa3, 0x9e, 0x05, 0x26, 0x78, 0xa5, 0x03, 0x8e, 0xb4, 0xd2, 0x31, 0x40, 0x06, 0x7d, 0x85,
        0x19, 0xa4, 0xe2, 0x8e, 0x28, 0x00, 0xa2, 0x8a, 0x28, 0x03,
    ];

    #[test]
    fn expands_a_stripped_thumbnail() {
        let jpeg = expand_inline_bytes(&STRIPPED).unwrap();
        assert_eq!(jpeg.len(), 623 + STRIPPED.len() - 3 + 2);
        assert_eq!((&jpeg[..4], jpeg[164], jpeg[166]), (&[0xff, 0xd8, 0xff, 0xe0][..], 0x0e, 0x28));
        assert_eq!((&jpeg[623..648], &jpeg[648..]), (&STRIPPED[3..], &[0xff, 0xd9][..]));
        // The digest of the reference decoder's output pins every header byte.
        let digest = [
            0xa3, 0xc4, 0xff, 0x73, 0xba, 0x1d, 0xdd, 0x11, 0x59, 0x9c, 0x82, 0x3b, 0x44, 0x81, 0x46, 0xd5, 0xe3, 0x2e,
            0x94, 0x29, 0xd0, 0x41, 0x08, 0xec, 0x7e, 0xfa, 0xfe, 0xcc, 0xea, 0x10, 0x02, 0x27,
        ];
        assert_eq!(crate::crypto::sha256(&[&jpeg]), digest);
    }

    #[test]
    fn refuses_short_or_unknown_payloads() {
        for payload in [&[][..], &[1], &[1, 2], &[2, 0x0e, 0x28, 0xa3], &[0, 1, 2]] {
            assert_eq!(expand_inline_bytes(payload), None, "{payload:?}");
        }
        let empty_scan = expand_inline_bytes(&[1, 2, 3]).unwrap();
        assert_eq!((empty_scan.len(), empty_scan[164], empty_scan[166]), (625, 2, 3));
    }

    #[test]
    fn decodes_a_vector_thumbnail_path() {
        let encoded = [
            0x1a, 0x00, 0xb2, 0x04, 0xdc, 0x47, 0x03, 0x81, 0x73, 0x55, 0x48, 0x01, 0x46, 0x05, 0x44, 0x45, 0x4f, 0x8b,
            0x52, 0x8d, 0x4e, 0x8a, 0x5d, 0x8b, 0x6e, 0x8b, 0x71, 0x81, 0x4c, 0x4d, 0x07, 0x47, 0x50, 0x02, 0x81, 0x46,
            0x84, 0x4b, 0x83, 0x50, 0x80, 0x44, 0x67, 0x47, 0x01, 0x6b, 0x47, 0x08, 0x43, 0x44, 0x4f, 0x53, 0x43, 0x56,
            0x8e, 0x44, 0x97, 0x94, 0x9d, 0x9c, 0x82, 0x84, 0x89, 0x45, 0x8b, 0x42, 0x88, 0x8a, 0xa6, 0xb3, 0xa8, 0xbc,
            0x81, 0x8a, 0x81, 0xad, 0x82, 0xae, 0x87, 0x88, 0xa5, 0x67, 0xab, 0x6a, 0x92, 0x49, 0xaa, 0x42, 0xb6, 0x4e,
            0x8d, 0x4e, 0x5e, 0x80, 0x69, 0x43, 0x45, 0x41, 0x4a, 0x46, 0x4d, 0x49, 0x52, 0x51, 0x8a, 0x46, 0x89, 0x47,
            0x4b, 0x4f, 0x5f, 0x5b, 0x6d, 0x67, 0x64, 0x5f, 0x46, 0x06, 0x48, 0x08, 0x63, 0x4d, 0x03, 0xa1, 0x70, 0x89,
            0x06, 0x81, 0x8b, 0x01, 0x48, 0x87, 0x44, 0x8a, 0x4c, 0x91, 0x51, 0x98, 0x51, 0x87, 0x03, 0x81, 0x89, 0x04,
            0x8f, 0x88, 0x86, 0x90, 0x8e, 0x95, 0x97, 0x81, 0x81, 0x82, 0x8b, 0x84, 0x8c, 0x98, 0x8d, 0xb2, 0x93, 0x87,
            0x00, 0xa9, 0xab, 0xad, 0x8e, 0x8a, 0x07, 0x69, 0x8c, 0x05, 0x5a, 0x89, 0x75, 0x84, 0x48, 0x00, 0x87, 0x46,
            0x80, 0x8c, 0x86, 0x92, 0x86, 0x90, 0x81, 0xa4, 0x4b, 0xb2, 0x81, 0x95, 0x91, 0x9b, 0xab, 0xab, 0xbf, 0x95,
            0x99, 0x87, 0x03, 0xb9, 0x87, 0x00, 0x89, 0x04, 0x42, 0xac, 0x4e, 0x04, 0xb5, 0x51, 0x04, 0xb3, 0x4a, 0x81,
            0x46, 0x86, 0x4a, 0x8b, 0x41, 0x81, 0x43, 0x44, 0x45, 0x43, 0x51, 0x85, 0x5c, 0x8c, 0x6f, 0x8c,
        ];
        let expected = "M260,504c-73,1-51-21-81-65-4-5-15,11-18,13-14,10-29,11-46,11-49,1-12-137-7-162,1-6,4-11,3-16,0-4-39-\
            71-43-78-3-4-15-19-3-22,14-4,23,20,29,28,2,4,9-5,11-2,8,10,38,51,40,60,1,10,1,45,2,46,7,8,37-39,43-4\
            2,18-9,42-2,54-14,13-14-30,0-41-3-5-1-10-6-13-9-18-17,10-6,9-7-11-15-31-27-45-39-36-31-66-88-35-133,\
            33-48,96,1,111-8,7-4,10-12,17-17,24-17,73,1,94,15,8,6,16,14,21,23,1,1,2,11,4,12,24,13,50,19,70,41,43\
            ,45,14,107-41,125-26,9-53,4-80,7-6,0,12,6,18,6,16,1,36-11,50,1,21,17,27,43,43,63,21,25,73,57,70,94-2\
            ,44-144,53-174,51-10,1-6,6-10,11-1,1-3-4-5-3-17,5-28,12-47,12z";
        assert_eq!(decode_svg_path(&encoded), expected);
        assert_eq!(decode_svg_path(&[]), "Mz");
        assert_eq!(decode_svg_path(&[0x3f, 0x40, 0x80, 0xbf, 0xc0, 0xff]), "M63-0,0,63A,z");
        for byte in 0..=u8::MAX {
            let path = decode_svg_path(&[byte]);
            assert!(path.starts_with('M') && path.ends_with('z') && path.len() > 2, "{byte}");
        }
    }
}
