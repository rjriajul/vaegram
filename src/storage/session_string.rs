//! V3 layout, big-endian: version 3 (u8), dc_id (u8), api_id (u32), test_mode (bool),
//! auth_key (256 bytes), user_id (u64), is_bot (bool), port (u16), server address (48 bytes),
//! then a CRC32 of all that (little-endian), base64url without padding.
//!
//! Written as `VAE_…`. Also reads legacy strings (dc_id, api_id, test_mode, auth_key, user_id, is_bot; no prefix, no CRC).

use super::SessionData;

pub const PREFIX: &str = "VAE_";

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const V3_LEN: usize = 322;
const LEGACY_LEN: usize = 271;

pub fn export(s: &SessionData) -> String {
    let mut b = Vec::with_capacity(V3_LEN + 4);
    b.push(3);
    b.push(s.dc_id as u8);
    b.extend_from_slice(&(s.api_id as u32).to_be_bytes());
    b.push(0);
    b.extend_from_slice(&s.auth_key);
    b.extend_from_slice(&(s.user_id as u64).to_be_bytes());
    b.push(s.is_bot as u8);
    b.extend_from_slice(&[0; 2 + 48]);
    b.extend_from_slice(&crc32fast::hash(&b).to_le_bytes());
    format!("{PREFIX}{}", encode(&b))
}

pub fn parse(s: &str) -> Result<SessionData, &'static str> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let body = s.strip_prefix(PREFIX).unwrap_or(&s);
    let raw = decode(body).ok_or("session string has characters outside base64url")?;
    let be_u32 = |b: &[u8]| u32::from_be_bytes(b.try_into().unwrap());
    let be_u64 = |b: &[u8]| u64::from_be_bytes(b.try_into().unwrap());

    let (dc_id, api_id, test_mode, auth_key, user_id, is_bot) = match raw.len() {
        n if n == V3_LEN + 4 => {
            if crc32fast::hash(&raw[..V3_LEN]).to_le_bytes() != raw[V3_LEN..] {
                return Err("session string is damaged (CRC mismatch); export it again");
            }
            if raw[0] != 3 {
                return Err("unknown session string version");
            }
            (raw[1], be_u32(&raw[2..6]), raw[6], &raw[7..263], be_u64(&raw[263..271]), raw[271])
        }
        LEGACY_LEN => (raw[0], be_u32(&raw[1..5]), raw[5], &raw[6..262], be_u64(&raw[262..270]), raw[270]),
        _ => return Err("not a V3 or legacy session string"),
    };
    if test_mode != 0 {
        return Err("test-server sessions aren't supported");
    }
    Ok(SessionData {
        dc_id: dc_id as i32,
        api_id: api_id as i32,
        auth_key: auth_key.try_into().unwrap(),
        user_id: user_id as i64,
        is_bot: is_bot != 0,
    })
}

fn encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(ALPHABET[(n >> (18 - 6 * i)) as usize & 63] as char);
        }
    }
    out
}

fn decode(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes() {
        acc = (acc << 6) | ALPHABET.iter().position(|&a| a == c)? as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}
