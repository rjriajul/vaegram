//! V4 layout, big-endian: version 4 (u8), dc_id (u8), api_id (u32), test_mode (bool),
//! auth_key (256 bytes), user_id (u64), is_bot (bool), port (u16, 0 when unknown), server IP (16 bytes,
//! IPv4 as IPv4-mapped IPv6), then a CRC32 of all that (little-endian), base64url without padding.
//!
//! Written as `VAE_…`. Also reads V3 (same fields up to is_bot, then an unused port and 48-byte address)
//! and legacy strings (dc_id, api_id, test_mode, auth_key, user_id, is_bot; no prefix, no CRC).

use std::net::{IpAddr, Ipv6Addr, SocketAddr};

use super::SessionData;
use crate::utils::{base64url_decode, base64url_encode};

pub const PREFIX: &str = "VAE_";

const V4_LEN: usize = 290;
const V3_LEN: usize = 322;
const LEGACY_LEN: usize = 271;

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
    let body = s.strip_prefix(PREFIX).unwrap_or(&s);
    let raw = base64url_decode(body).ok_or("session string has characters outside base64url")?;
    let be_u32 = |b: &[u8]| u32::from_be_bytes(b.try_into().unwrap());
    let be_u64 = |b: &[u8]| u64::from_be_bytes(b.try_into().unwrap());

    let (f, addr) = match raw.len() {
        LEGACY_LEN => (&raw[..], None),
        n if n == V4_LEN + 4 || n == V3_LEN + 4 => {
            let (body, crc) = raw.split_at(n - 4);
            if crc32fast::hash(body).to_le_bytes() != *crc {
                return Err("session string is damaged (CRC mismatch); export it again");
            }
            let addr = match body[0] {
                3 if n == V3_LEN + 4 => None,
                4 if n == V4_LEN + 4 => {
                    let port = u16::from_be_bytes([body[272], body[273]]);
                    let ip = Ipv6Addr::from(<[u8; 16]>::try_from(&body[274..]).unwrap()).to_canonical();
                    (port != 0).then_some(SocketAddr::new(ip, port))
                }
                _ => return Err("unknown session string version"),
            };
            (&body[1..], addr)
        }
        _ => return Err("not a V4, V3 or legacy session string"),
    };
    Ok(SessionData {
        dc_id: f[0] as i32,
        test_mode: f[5] != 0,
        addr,
        api_id: be_u32(&f[1..5]) as i32,
        auth_key: f[6..262].try_into().unwrap(),
        user_id: be_u64(&f[262..270]) as i64,
        is_bot: f[270] != 0,
    })
}
