//! Session strings: `VAE_` + base64url (no padding) of a generation byte (1), tag-length-value fields
//! (`tag: u8`, `len: u16` LE, value) and a CRC32 (LE) of everything before it.
//!
//! | Tag  | Field     | Value                                                    |
//! |------|-----------|----------------------------------------------------------|
//! | 0x01 | dc_id     | u16 LE                                                   |
//! | 0x02 | auth_key  | 256 bytes                                                |
//! | 0x03 | user_id   | i64 LE, 0 before sign-in                                 |
//! | 0x04 | flags     | u8: bit 0 test mode, bit 1 bot; other bits are ignored   |
//! | 0x05 | api_id    | i32 LE, left out when 0                                  |
//! | 0x06 | address   | port u16 LE + 4 or 16 IP bytes, left out when unknown    |
//!
//! A reader skips unknown tags below 0x80 and refuses unknown tags from 0x80 up, so a field can be added
//! without a new generation. Strings written by older vaegram versions are refused: their owners log in
//! again.
//!
//! Imports, read but never written:
//! - `WZ_` + the 322-byte version 3 layout with a CRC, or the 290-byte version 2 layout with or without one
//!   (also accepted without the prefix);
//! - unprefixed 271-, 267- and 263-byte strings (no address; the shorter ones have no api_id);
//! - `1` + base64url of dc (u8), an IPv4 or IPv6 address, port (u16 BE) and the key. These carry no user id:
//!   the client asks who it is once connected.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};

use super::SessionData;
use crate::utils::{base64url_decode, base64url_encode};

/// What every session string `export` writes starts with.
pub const PREFIX: &str = "VAE_";
const WZ_PREFIX: &str = "WZ_";
const ADDRESS_FIRST_PREFIX: &str = "1";

// Generations 2 to 4 are the version bytes of the old fixed layouts, so the next generation is 5.
const GENERATION: u8 = 1;
const MAX_DECODED: usize = 4096;
// The longest input worth decoding: the prefix plus base64 of MAX_DECODED bytes, with room for padding.
const MAX_ENCODED: usize = PREFIX.len() + MAX_DECODED / 3 * 4 + 8;

const DC_ID: u8 = 0x01;
const AUTH_KEY: u8 = 0x02;
const USER_ID: u8 = 0x03;
const FLAGS: u8 = 0x04;
const API_ID: u8 = 0x05;
const ADDRESS: u8 = 0x06;
const CRITICAL: u8 = 0x80;

const TEST_MODE: u8 = 1;
const IS_BOT: u8 = 2;

const V3_LEN: usize = 322;
const V2_LEN: usize = 290;
const LEGACY_LEN: usize = 271;
const LEGACY_NO_API_ID_LEN: usize = 267;
const LEGACY_U32_USER_LEN: usize = 263;
const ADDRESS_FIRST_V4_LEN: usize = 263;
const ADDRESS_FIRST_V6_LEN: usize = 275;

const OLD_VAEGRAM: &str = "this session string is from an older vaegram; log in again";
pub(crate) const NEWER_VAEGRAM: &str = "this session string needs a newer vaegram";
const DAMAGED: &str = "session string is damaged (CRC mismatch); export it again";
const MALFORMED: &str = "session string has a malformed field";
const NOT_BASE64: &str = "session string has characters outside base64url";
const UNKNOWN: &str = "not a session string this version can read";

/// `s` as a session string: `VAE_` and generation 1, the only form vaegram writes. It holds the auth key, so it is
/// the login itself.
pub fn export(s: &SessionData) -> String {
    let mut b = encode_fields(s);
    b.extend_from_slice(&crc32fast::hash(&b).to_le_bytes());
    format!("{PREFIX}{}", base64url_encode(&b))
}

/// The generation byte and the fields, without the CRC: session records in a file keep exactly this.
pub(crate) fn encode_fields(s: &SessionData) -> Vec<u8> {
    let mut b = Vec::with_capacity(1 + 3 * 6 + 2 + 256 + 8 + 1 + 4 + 18 + 4);
    b.push(GENERATION);
    field(&mut b, DC_ID, &(s.dc_id as u16).to_le_bytes());
    field(&mut b, AUTH_KEY, &s.auth_key);
    field(&mut b, USER_ID, &s.user_id.to_le_bytes());
    field(&mut b, FLAGS, &[(s.test_mode as u8 * TEST_MODE) | (s.is_bot as u8 * IS_BOT)]);
    if s.api_id != 0 {
        field(&mut b, API_ID, &s.api_id.to_le_bytes());
    }
    if let Some(addr) = s.addr {
        let mut value = addr.port().to_le_bytes().to_vec();
        match addr.ip() {
            IpAddr::V4(ip) => value.extend_from_slice(&ip.octets()),
            IpAddr::V6(ip) => value.extend_from_slice(&ip.octets()),
        }
        field(&mut b, ADDRESS, &value);
    }
    b
}

fn field(b: &mut Vec<u8>, tag: u8, value: &[u8]) {
    b.push(tag);
    b.extend_from_slice(&(value.len() as u16).to_le_bytes());
    b.extend_from_slice(value);
}

/// Reads a session string in any form the [module](self) lists; whitespace anywhere is ignored. Fails with a
/// message to show the user when the string is damaged, from an older or newer vaegram, or not a session string.
pub fn parse(s: &str) -> Result<SessionData, &'static str> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).take(MAX_ENCODED + 1).collect();
    if s.len() > MAX_ENCODED {
        return Err(UNKNOWN);
    }
    if let Some(body) = s.strip_prefix(PREFIX) {
        return parse_generation(&decode(body)?);
    }
    if let Some(body) = s.strip_prefix(ADDRESS_FIRST_PREFIX) {
        return parse_address_first(&decode(body)?);
    }
    parse_import(&decode(s.strip_prefix(WZ_PREFIX).unwrap_or(&s))?)
}

/// Whether `s` comes from another library rather than from `export`.
pub(crate) fn is_import(s: &str) -> bool {
    !s.trim_start().starts_with(PREFIX)
}

fn decode(body: &str) -> Result<Vec<u8>, &'static str> {
    base64url_decode(body.trim_end_matches('=')).ok_or(NOT_BASE64)
}

fn split_crc(raw: &[u8]) -> Option<&[u8]> {
    let (body, crc) = raw.split_at_checked(raw.len().checked_sub(4)?)?;
    (crc32fast::hash(body).to_le_bytes() == *crc).then_some(body)
}

fn parse_generation(raw: &[u8]) -> Result<SessionData, &'static str> {
    if raw.len() > MAX_DECODED {
        return Err(UNKNOWN);
    }
    decode_fields(split_crc(raw).ok_or(DAMAGED)?)
}

pub(crate) fn decode_fields(body: &[u8]) -> Result<SessionData, &'static str> {
    match body.first() {
        Some(&GENERATION) => {}
        Some(2..=4) => return Err(OLD_VAEGRAM),
        Some(0) | None => return Err(UNKNOWN),
        Some(_) => return Err(NEWER_VAEGRAM),
    }
    let mut rest = &body[1..];
    let mut seen = [false; 256];
    let (mut dc_id, mut auth_key, mut user_id, mut flags) = (None, None, None, None);
    let mut s = SessionData::new(0, [0; 256]);
    while !rest.is_empty() {
        let (head, tail) = rest.split_first_chunk::<3>().ok_or(MALFORMED)?;
        let [tag, l0, l1] = *head;
        let (value, tail) = tail.split_at_checked(u16::from_le_bytes([l0, l1]) as usize).ok_or(MALFORMED)?;
        rest = tail;
        if std::mem::replace(&mut seen[tag as usize], true) {
            return Err(MALFORMED);
        }
        match tag {
            DC_ID => dc_id = Some(u16::from_le_bytes(value.try_into().map_err(|_| MALFORMED)?)),
            AUTH_KEY => auth_key = Some(<[u8; 256]>::try_from(value).map_err(|_| MALFORMED)?),
            USER_ID => user_id = Some(i64::from_le_bytes(value.try_into().map_err(|_| MALFORMED)?)),
            FLAGS => flags = Some(<[u8; 1]>::try_from(value).map_err(|_| MALFORMED)?[0]),
            API_ID => s.api_id = i32::from_le_bytes(value.try_into().map_err(|_| MALFORMED)?),
            ADDRESS => s.addr = Some(address(value).ok_or(MALFORMED)?),
            CRITICAL..=u8::MAX => return Err(NEWER_VAEGRAM),
            _ => {}
        }
    }
    let (Some(dc_id), Some(auth_key), Some(user_id), Some(flags)) = (dc_id, auth_key, user_id, flags) else {
        return Err(MALFORMED);
    };
    s.dc_id = dc_id as i32;
    s.auth_key = auth_key;
    s.user_id = user_id;
    s.test_mode = flags & TEST_MODE != 0;
    s.is_bot = flags & IS_BOT != 0;
    Ok(s)
}

fn address(value: &[u8]) -> Option<SocketAddr> {
    let (port, ip) = value.split_first_chunk::<2>()?;
    let ip = match ip.len() {
        4 => IpAddr::V4(Ipv4Addr::from(<[u8; 4]>::try_from(ip).ok()?)),
        16 => IpAddr::V6(Ipv6Addr::from(<[u8; 16]>::try_from(ip).ok()?)),
        _ => return None,
    };
    Some(SocketAddr::new(ip, u16::from_le_bytes(*port)))
}

fn parse_address_first(raw: &[u8]) -> Result<SessionData, &'static str> {
    let ip_len = match raw.len() {
        ADDRESS_FIRST_V4_LEN => 4,
        ADDRESS_FIRST_V6_LEN => 16,
        _ => return Err(UNKNOWN),
    };
    let (ip, rest) = raw[1..].split_at(ip_len);
    let ip = match ip_len {
        4 => IpAddr::V4(Ipv4Addr::from(<[u8; 4]>::try_from(ip).unwrap())),
        _ => IpAddr::V6(Ipv6Addr::from(<[u8; 16]>::try_from(ip).unwrap())),
    };
    let (port, key) = rest.split_at(2);
    let mut s = SessionData::new(raw[0] as i32, key.try_into().unwrap());
    let port = u16::from_be_bytes([port[0], port[1]]);
    s.addr = (port != 0).then(|| SocketAddr::new(ip, port));
    Ok(s)
}

fn parse_import(raw: &[u8]) -> Result<SessionData, &'static str> {
    let be_u32 = |b: &[u8]| u32::from_be_bytes(b.try_into().unwrap());
    let be_u64 = |b: &[u8]| u64::from_be_bytes(b.try_into().unwrap());
    let (f, checked) = match raw.len() {
        n if n == V2_LEN + 4 || n == V3_LEN + 4 => (split_crc(raw).ok_or(DAMAGED)?, true),
        _ => (raw, false),
    };
    let len = f.len();
    let (version, mut rest) = match (len, f.first(), checked) {
        (LEGACY_LEN | LEGACY_NO_API_ID_LEN | LEGACY_U32_USER_LEN, _, false) => (0, f),
        (V2_LEN, Some(&v @ 2), _) | (V3_LEN, Some(&v @ 3), true) => (v, &f[1..]),
        (V2_LEN, Some(4), true) => return Err(OLD_VAEGRAM),
        _ => return Err(UNKNOWN),
    };
    let mut take = |n: usize| {
        let (head, tail) = rest.split_at(n);
        rest = tail;
        head
    };
    let mut s = SessionData::new(take(1)[0] as i32, [0; 256]);
    s.api_id = if len < LEGACY_LEN { 0 } else { be_u32(take(4)) as i32 };
    s.test_mode = take(1)[0] != 0;
    s.auth_key = take(256).try_into().unwrap();
    s.user_id = if len == LEGACY_U32_USER_LEN { be_u32(take(4)) as i64 } else { be_u64(take(8)) as i64 };
    s.is_bot = take(1)[0] != 0;
    let port = if version == 0 { 0 } else { u16::from_be_bytes(take(2).try_into().unwrap()) };
    let ip: Option<IpAddr> = std::str::from_utf8(rest).ok().and_then(|a| a.trim_end_matches('\0').parse().ok());
    s.addr = ip.filter(|_| port != 0).map(|ip| SocketAddr::new(ip, port));
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> SessionData {
        let mut s = SessionData::new(4, std::array::from_fn(|i| i as u8));
        (s.api_id, s.user_id, s.is_bot, s.test_mode) = (12345, 777000, true, true);
        s.addr = Some("149.154.167.40:443".parse().unwrap());
        s
    }

    fn fields(s: &SessionData) -> (i32, bool, Option<SocketAddr>, i32, [u8; 256], i64, bool) {
        (s.dc_id, s.test_mode, s.addr, s.api_id, s.auth_key, s.user_id, s.is_bot)
    }

    fn raw(s: &str) -> Vec<u8> {
        base64url_decode(s.strip_prefix(PREFIX).unwrap()).unwrap()
    }

    // Re-seals `raw` (generation and fields, CRC stripped) as a string.
    fn seal(mut raw: Vec<u8>) -> String {
        raw.extend_from_slice(&crc32fast::hash(&raw).to_le_bytes());
        format!("{PREFIX}{}", base64url_encode(&raw))
    }

    fn unsealed(s: &SessionData) -> Vec<u8> {
        let mut b = raw(&export(s));
        b.truncate(b.len() - 4);
        b
    }

    #[test]
    fn exports_and_parses_generation_1() {
        let mut s = data();
        for addr in [None, Some("[2001:67c:4e8:f002::a]:443".parse().unwrap()), s.addr] {
            s.addr = addr;
            for api_id in [0, 12345] {
                s.api_id = api_id;
                let exported = export(&s);
                assert!(exported.starts_with("VAE_A"), "{exported}");
                assert_eq!(fields(&parse(&exported).unwrap()), fields(&s));
            }
        }
        let b = unsealed(&s);
        assert_eq!(b[0], GENERATION);
        let tags: Vec<u8> = {
            let (mut rest, mut tags) = (&b[1..], Vec::new());
            while let [tag, l0, l1, tail @ ..] = rest {
                tags.push(*tag);
                rest = &tail[u16::from_le_bytes([*l0, *l1]) as usize..];
            }
            tags
        };
        assert_eq!(tags, [DC_ID, AUTH_KEY, USER_ID, FLAGS, API_ID, ADDRESS], "ascending, each once");
    }

    #[test]
    fn unknown_low_tag_is_skipped() {
        let mut b = unsealed(&data());
        field(&mut b, 0x7f, b"later");
        assert_eq!(fields(&parse(&seal(b)).unwrap()), fields(&data()));
    }

    #[test]
    fn unknown_critical_tag_is_refused() {
        for tag in [CRITICAL, 0xff] {
            let mut b = unsealed(&data());
            field(&mut b, tag, &[1]);
            assert_eq!(parse(&seal(b)).err(), Some(NEWER_VAEGRAM));
        }
        let mut newer = unsealed(&data());
        newer[0] = 5;
        assert_eq!(parse(&seal(newer)).err(), Some(NEWER_VAEGRAM));
    }

    #[test]
    fn duplicate_or_missing_field_is_an_error() {
        let mut duplicate = unsealed(&data());
        field(&mut duplicate, USER_ID, &1i64.to_le_bytes());
        assert_eq!(parse(&seal(duplicate)).err(), Some(MALFORMED));

        let mut skipped_twice = unsealed(&data());
        field(&mut skipped_twice, 0x10, &[]);
        field(&mut skipped_twice, 0x10, &[]);
        assert_eq!(parse(&seal(skipped_twice)).err(), Some(MALFORMED));

        let s = data();
        let mut missing = vec![GENERATION];
        field(&mut missing, DC_ID, &4u16.to_le_bytes());
        field(&mut missing, AUTH_KEY, &s.auth_key);
        field(&mut missing, FLAGS, &[0]);
        assert_eq!(parse(&seal(missing)).err(), Some(MALFORMED));

        for (tag, value) in [(DC_ID, &[4][..]), (USER_ID, &[0; 4][..]), (FLAGS, &[][..]), (ADDRESS, &[0; 7][..])] {
            let mut b = vec![GENERATION];
            field(&mut b, tag, value);
            assert_eq!(parse(&seal(b)).err(), Some(MALFORMED), "tag {tag}");
        }

        let mut truncated = unsealed(&data());
        truncated.extend_from_slice(&[0x10, 9, 0, 1]);
        assert_eq!(parse(&seal(truncated)).err(), Some(MALFORMED));
    }

    #[test]
    fn damage_size_and_alphabet_are_checked() {
        let good = export(&data());
        let mut damaged = good.clone();
        damaged.replace_range(40..41, if &good[40..41] == "A" { "B" } else { "A" });
        assert_eq!(parse(&damaged).err(), Some(DAMAGED));
        // A damaged generation byte is damage, not a string from another version.
        for first in [0, 2, 4, 5, 0xff] {
            let mut b = raw(&good);
            b[0] = first;
            assert_eq!(parse(&format!("{PREFIX}{}", base64url_encode(&b))).err(), Some(DAMAGED), "{first}");
        }
        let oversized = format!("{PREFIX}{}", "A".repeat(MAX_ENCODED));
        assert_eq!(parse(&oversized).err(), Some(UNKNOWN));

        let mut huge = unsealed(&data());
        field(&mut huge, 0x10, &[0; 4000]);
        assert_eq!(parse(&seal(huge)).err(), Some(UNKNOWN));

        assert_eq!(parse("VAE_not*base64").err(), Some(NOT_BASE64));
        for short in ["VAE_", "VAE_AA", "VAE_AQ"] {
            assert!(parse(short).is_err(), "{short}");
        }
        assert_eq!(parse(&format!(" {}\n", good.replace('A', "A "))).map(|s| fields(&s)), Ok(fields(&data())));
    }

    #[test]
    fn unknown_flag_bits_are_ignored() {
        let s = data();
        let mut b = vec![GENERATION];
        field(&mut b, DC_ID, &4u16.to_le_bytes());
        field(&mut b, AUTH_KEY, &s.auth_key);
        field(&mut b, USER_ID, &s.user_id.to_le_bytes());
        field(&mut b, FLAGS, &[0b1111_1110]);
        let back = parse(&seal(b)).unwrap();
        assert_eq!((back.test_mode, back.is_bot, back.api_id, back.addr), (false, true, 0, None));
    }

    #[test]
    fn old_vae_strings_say_log_in_again() {
        let mut v4 = vec![4u8; 290];
        v4.extend_from_slice(&crc32fast::hash(&v4).to_le_bytes());
        let mut v3 = vec![3u8; 322];
        v3.extend_from_slice(&crc32fast::hash(&v3).to_le_bytes());
        for old in [&v4, &v3] {
            assert_eq!(parse(&format!("{PREFIX}{}", base64url_encode(old))).err(), Some(OLD_VAEGRAM));
        }
        assert_eq!(parse(&base64url_encode(&v4)).err(), Some(OLD_VAEGRAM));
        assert_eq!(parse(&format!("{WZ_PREFIX}{}", base64url_encode(&v4))).err(), Some(OLD_VAEGRAM));
    }

    fn address_first(ip: &[u8], port: u16) -> String {
        let mut b = vec![2];
        b.extend_from_slice(ip);
        b.extend_from_slice(&port.to_be_bytes());
        b.extend_from_slice(&[9; 256]);
        let encoded = base64url_encode(&b);
        format!("{ADDRESS_FIRST_PREFIX}{encoded}{}", "=".repeat((4 - encoded.len() % 4) % 4))
    }

    #[test]
    fn reads_address_first_ipv4_and_ipv6_strings() {
        let v4 = parse(&address_first(&[149, 154, 167, 51], 443)).unwrap();
        assert_eq!(fields(&v4), (2, false, Some("149.154.167.51:443".parse().unwrap()), 0, [9; 256], 0, false));
        let ip: Ipv6Addr = "2001:67c:4e8:f002::a".parse().unwrap();
        let v6 = parse(&address_first(&ip.octets(), 443)).unwrap();
        assert_eq!(v6.addr, Some("[2001:67c:4e8:f002::a]:443".parse().unwrap()));
        assert_eq!(parse(&address_first(&[127, 0, 0, 1], 0)).unwrap().addr, None);
        assert!(is_import(&address_first(&[127, 0, 0, 1], 443)));
        assert!(!is_import(&format!("  {}", export(&data()))));
    }

    #[test]
    fn address_first_strings_need_the_right_length() {
        let good = address_first(&[149, 154, 167, 51], 443);
        let raw = base64url_decode(good[1..].trim_end_matches('=')).unwrap();
        for len in [0, 1, 262, 264, 274, 276] {
            let mut b = raw.clone();
            b.resize(len, 0);
            assert_eq!(parse(&format!("1{}", base64url_encode(&b))).err(), Some(UNKNOWN), "{len}");
        }
    }

    #[test]
    fn resealed_garbage_never_panics() {
        // Mutations get a fresh CRC so they reach the field parser instead of stopping at the checksum.
        let seed = unsealed(&data());
        let mut state = 0x2545_f491_4f6c_dd1du64;
        let mut next = move |n: usize| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            (state % n as u64) as usize
        };
        for _ in 0..20_000 {
            let mut b = seed.clone();
            for _ in 0..=next(4) {
                let at = next(b.len());
                match next(3) {
                    0 => b[at] = next(256) as u8,
                    1 => b.truncate(at + 1),
                    _ => b.insert(at, next(256) as u8),
                }
            }
            b[0] = GENERATION;
            let _ = parse(&seal(b));
        }
    }

    #[test]
    fn address_first_and_unprefixed_strings_are_never_confused() {
        // An unprefixed string starts with its dc byte, so it encodes to `A` or `B`, never `1`.
        for dc in 0..8u8 {
            for len in [LEGACY_LEN, LEGACY_NO_API_ID_LEN, LEGACY_U32_USER_LEN, V2_LEN] {
                let mut b = vec![0; len];
                b[0] = dc;
                assert!(!base64url_encode(&b).starts_with(ADDRESS_FIRST_PREFIX));
            }
        }
        let same_length = vec![4; LEGACY_U32_USER_LEN];
        let unprefixed = parse(&base64url_encode(&same_length)).unwrap();
        assert_eq!((unprefixed.dc_id, unprefixed.addr), (4, None));
        let address_first = parse(&format!("1{}", base64url_encode(&same_length))).unwrap();
        assert_eq!(address_first.addr, Some("4.4.4.4:1028".parse().unwrap()));
    }
}
