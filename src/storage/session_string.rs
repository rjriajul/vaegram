//! V4 layout, big-endian: version 4 (u8), dc_id (u8), api_id (u32), test_mode (bool),
//! auth_key (256 bytes), user_id (u64), is_bot (bool), port (u16, 0 when unknown), server IP (16 bytes,
//! IPv4 as IPv4-mapped IPv6), then a CRC32 of all that (little-endian), base64url without padding.
//!
//! Written as `VAE_…`. Also reads, with either `VAE_` or wzgram's `WZ_` prefix: V3 and wzgram's V2
//! (same fields up to the port, then the address as NUL-padded ASCII in 48 or 16 bytes; early wzgram
//! wrote V2 without the CRC) and Pyrogram's unprefixed legacy strings (no version, no address, no CRC;
//! the oldest lack api_id).

use std::net::{IpAddr, Ipv6Addr, SocketAddr};

use super::SessionData;
use crate::utils::{base64url_decode, base64url_encode};

pub const PREFIX: &str = "VAE_";
const WZ_PREFIX: &str = "WZ_";

const V4_LEN: usize = 290;
const V3_LEN: usize = 322;
const LEGACY_LEN: usize = 271;
const LEGACY_NO_API_ID_LEN: usize = 267;
const LEGACY_U32_USER_LEN: usize = 263;

pub fn export(s: &SessionData) -> String {
    let (port, ip) = match s.addr {
        Some(addr) => match addr.ip() {
            IpAddr::V4(v4) => (addr.port(), v4.to_ipv6_mapped()),
            IpAddr::V6(v6) => (addr.port(), v6),
        },
        None => (0, Ipv6Addr::UNSPECIFIED),
    };
    let mut b = Vec::with_capacity(V4_LEN + 4);
    b.push(4);
    b.push(s.dc_id as u8);
    b.extend_from_slice(&(s.api_id as u32).to_be_bytes());
    b.push(s.test_mode as u8);
    b.extend_from_slice(&s.auth_key);
    b.extend_from_slice(&(s.user_id as u64).to_be_bytes());
    b.push(s.is_bot as u8);
    b.extend_from_slice(&port.to_be_bytes());
    b.extend_from_slice(&ip.octets());
    b.extend_from_slice(&crc32fast::hash(&b).to_le_bytes());
    format!("{PREFIX}{}", base64url_encode(&b))
}

pub fn parse(s: &str) -> Result<SessionData, &'static str> {
    let s: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    let body = s.strip_prefix(PREFIX).or_else(|| s.strip_prefix(WZ_PREFIX)).unwrap_or(&s);
    let raw = base64url_decode(body.trim_end_matches('=')).ok_or("session string has characters outside base64url")?;
    let be_u32 = |b: &[u8]| u32::from_be_bytes(b.try_into().unwrap());
    let be_u64 = |b: &[u8]| u64::from_be_bytes(b.try_into().unwrap());

    let (f, checked) = match raw.len() {
        n if n == V4_LEN + 4 || n == V3_LEN + 4 => {
            let (body, crc) = raw.split_at(n - 4);
            if crc32fast::hash(body).to_le_bytes() != *crc {
                return Err("session string is damaged (CRC mismatch); export it again");
            }
            (body, true)
        }
        _ => (&raw[..], false),
    };
    let len = f.len();
    let (version, mut rest) = match (len, f.first(), checked) {
        (LEGACY_LEN | LEGACY_NO_API_ID_LEN | LEGACY_U32_USER_LEN, _, false) => (0, f),
        (V4_LEN, Some(&v @ 2), _) | (V4_LEN, Some(&v @ 4), true) | (V3_LEN, Some(&v @ 3), true) => (v, &f[1..]),
        _ => return Err("not a V4, V3, V2 or legacy session string"),
    };
    let mut take = |n: usize| {
        let (head, tail) = rest.split_at(n);
        rest = tail;
        head
    };
    let dc_id = take(1)[0] as i32;
    let api_id = if len < LEGACY_LEN { 0 } else { be_u32(take(4)) as i32 };
    let test_mode = take(1)[0] != 0;
    let auth_key = take(256).try_into().unwrap();
    let user_id = if len == LEGACY_U32_USER_LEN { be_u32(take(4)) as i64 } else { be_u64(take(8)) as i64 };
    let is_bot = take(1)[0] != 0;
    let port = if version == 0 { 0 } else { u16::from_be_bytes(take(2).try_into().unwrap()) };
    let ip = match version {
        4 => Some(IpAddr::from(<[u8; 16]>::try_from(rest).unwrap()).to_canonical()),
        _ => std::str::from_utf8(rest).ok().and_then(|a| a.trim_end_matches('\0').parse().ok()),
    };
    let addr = ip.filter(|_| port != 0).map(|ip| SocketAddr::new(ip, port));
    Ok(SessionData { dc_id, test_mode, addr, api_id, auth_key, user_id, is_bot })
}
