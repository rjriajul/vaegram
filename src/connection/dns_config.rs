//! Telegram's fallback config: a few data center addresses, published as DNS TXT records and signed with a key of
//! its own, for when the usual addresses can't be reached.

use std::cmp::Reverse;
use std::io;
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::http1::{self, Head};
use super::tls;
use crate::crypto::rsa::PublicKey;
use crate::crypto::{aes, sha256};
use crate::errors::{Error, Result};
use crate::raw::Deserializable;
use crate::utils::base64_decode;

/// The key the config is signed with; it signs nothing else.
const DNS_KEY: &str = "-----BEGIN RSA PUBLIC KEY-----
MIIBCgKCAQEAyr+18Rex2ohtVy8sroGPBwXD3DOoKCSpjDqYoXgCqB7ioln4eDCF
fOBUlfXUEvM/fnKCpF46VkAftlb4VuPDeQSS/ZxZYEGqHaywlroVnXHIjgqoxiAd
192xRGreuXIaUKmkwlM9JID9WS2jUsTpzQ91L8MEPLJ/4zrBwZua8W5fECwCCh2c
9G5IzzBm+otMS/YKwmR1olzRCyEkyAEjXWqBI9Ftv5eG8m0VkBzOG655WIYdyV0H
fDK/NWcvGqa0w/nriMD6mDjKOryamw0OP9QuYgMN0C9xMW9y8SmP4h92OAWodTYg
Y1hZCxdv6cs5UnW9+PWvS+WIbkh+GaWYxwIDAQAB
-----END RSA PUBLIC KEY-----";

// 256 bytes in base64.
const ENCODED_LEN: usize = 344;
// The first 208 decrypted bytes are signed; the next 16 are the start of their SHA-256.
const SIGNED_LEN: usize = 208;

const HELP_CONFIG_SIMPLE: u32 = 0x5a59_2a6c;
const ACCESS_POINT_RULE: u32 = 0x4679_b65f;
const IP_PORT: u32 = 0xd433_ad73;
const IP_PORT_SECRET: u32 = 0x3798_2646;

/// DNS-over-HTTPS JSON endpoints, tried in turn.
const RESOLVERS: [(&str, &str); 2] = [("dns.google", "/resolve"), ("mozilla.cloudflare-dns.com", "/dns-query")];
const RESOLVER_TIMEOUT: Duration = Duration::from_secs(10);
// A TXT answer is a few hundred bytes.
const MAX_BODY: usize = 64 << 10;

/// The config Telegram publishes in DNS for clients that can't reach its usual addresses. Its RSA signature is
/// checked, so a spoofed DNS answer can't produce one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnsConfig {
    pub(crate) date: i32,
    pub(crate) expires: i32,
    pub(crate) rules: Vec<AccessPointRule>,
}

/// The addresses of one data center, for the phone numbers `phone_prefix_rules` names (all of them when empty).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AccessPointRule {
    /// The phone number prefixes this rule is for, as Telegram writes them; empty for every number.
    pub phone_prefix_rules: String,
    /// The data center these addresses belong to.
    pub dc_id: i32,
    /// The data center's addresses.
    pub ips: Vec<IpPort>,
}

/// An address of a data center. One with a `secret` only answers MTProxy-style obfuscation keyed with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpPort {
    /// The IPv4 address and port.
    pub addr: SocketAddrV4,
    /// The obfuscation secret this address needs; `None` for a plain connection.
    pub secret: Option<Vec<u8>>,
}

impl DnsConfig {
    /// When the config was made, in Unix seconds.
    pub fn date(&self) -> i32 {
        self.date
    }

    /// When the config stops being valid, in Unix seconds.
    pub fn expires(&self) -> i32 {
        self.expires
    }

    /// The data centers' addresses, in the order the config lists them.
    pub fn rules(&self) -> &[AccessPointRule] {
        &self.rules
    }

    /// The addresses of data center `dc_id` that take a plain connection, the ones that need a secret left out;
    /// none once the config has expired at `now` (Unix seconds).
    pub(crate) fn plain_addresses(&self, dc_id: i32, now: u64) -> Vec<SocketAddr> {
        if u64::try_from(self.expires).unwrap_or(0) < now {
            return Vec::new();
        }
        let rules = self.rules.iter().filter(|r| r.dc_id == dc_id);
        rules.flat_map(|r| &r.ips).filter(|ip| ip.secret.is_none()).map(|ip| SocketAddr::V4(ip.addr)).collect()
    }

    /// Reads the config from the strings of its TXT records, in any order: they're joined longest first, as
    /// Telegram splits it.
    pub fn parse<S: AsRef<str>>(txt: &[S]) -> Result<DnsConfig> {
        let mut parts: Vec<&str> = txt.iter().map(AsRef::as_ref).collect();
        parts.sort_by_key(|p| Reverse(p.len()));
        let encoded: String =
            parts.concat().chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '/' | '=')).collect();
        if encoded.len() != ENCODED_LEN {
            return Err(Error::Unexpected("the DNS config isn't 344 base64 characters"));
        }
        let encrypted = base64_decode(&encoded)
            .filter(|data| data.len() == 256)
            .ok_or(Error::Unexpected("the DNS config isn't valid base64"))?;
        decode(&decrypt(&encrypted)?)
    }

    /// Fetches the config over DNS-over-HTTPS, from Google's resolver and then Cloudflare's; `test` asks for the
    /// test servers' config. The resolvers' certificates are checked, and so is the config's signature.
    pub async fn fetch(test: bool) -> Result<DnsConfig> {
        let name = if test { "tapv3.stel.com" } else { "apv3.stel.com" };
        let mut last = Error::Unexpected("no DNS resolver answered");
        for (host, path) in RESOLVERS {
            let answer = tokio::time::timeout(RESOLVER_TIMEOUT, resolve_txt(host, path, name)).await;
            match answer.map_err(|_| Error::Timeout).and_then(|txt| txt).and_then(|txt| DnsConfig::parse(&txt)) {
                Ok(config) => return Ok(config),
                Err(error) => last = error,
            }
        }
        Err(last)
    }
}

/// Opens the signed blob: RSA with the DNS key, then AES-256-CBC keyed by its first 32 bytes; returns the TL that
/// the length word at the start of the signed part covers.
fn decrypt(encrypted: &[u8]) -> Result<Vec<u8>> {
    let key = PublicKey::from_pem(DNS_KEY).ok_or(Error::Unexpected("the DNS config key doesn't parse"))?;
    let mut data = key.raw_public(encrypted);
    let (head, body) = data.split_at_mut(32);
    let mut aes_key = [0; 32];
    aes_key.copy_from_slice(head);
    let mut iv = [0; 16];
    iv.copy_from_slice(&head[16..]);
    cbc_decrypt(body, &aes_key, iv)?;
    let (signed, hash) = body.split_at(SIGNED_LEN);
    if sha256(&[signed])[..16] != *hash {
        return Err(Error::Security("the DNS config's hash doesn't match"));
    }
    let len = u32::from_le_bytes([signed[0], signed[1], signed[2], signed[3]]);
    let len = usize::try_from(len).ok().filter(|len| (8..=SIGNED_LEN).contains(len));
    let len = len.ok_or(Error::Security("the DNS config's length is out of range"))?;
    Ok(signed[4..len].to_vec())
}

/// AES-256-CBC decryption in place, built from IGE: one-block IGE with the previous ciphertext block as its first
/// half and zeros as its second is exactly one CBC step.
fn cbc_decrypt(data: &mut [u8], key: &[u8; 32], iv: [u8; 16]) -> Result<()> {
    let mut previous = iv;
    for block in data.chunks_mut(16) {
        let mut cipher = [0; 16];
        cipher.copy_from_slice(block);
        let mut ige_iv = [0; 32];
        ige_iv[..16].copy_from_slice(&previous);
        aes::ige256_decrypt(block, key, &ige_iv).map_err(|_| Error::Unexpected("the DNS config isn't whole blocks"))?;
        previous = cipher;
    }
    Ok(())
}

/// `help.configSimple date:int expires:int rules:vector<AccessPointRule>`, whose vectors are bare.
fn decode(mut tl: &[u8]) -> Result<DnsConfig> {
    let buf = &mut tl;
    if u32::deserialize(buf)? != HELP_CONFIG_SIMPLE {
        return Err(Error::Unexpected("the DNS config isn't a help.configSimple"));
    }
    let date = i32::deserialize(buf)?;
    let expires = i32::deserialize(buf)?;
    let rules = (0..count(buf)?).map(|_| rule(buf)).collect::<Result<_>>()?;
    Ok(DnsConfig { date, expires, rules })
}

/// A bare vector's length; each item takes at least four bytes, so a longer count can't be real.
fn count(buf: &mut &[u8]) -> Result<usize> {
    let count = usize::try_from(u32::deserialize(buf)?).unwrap_or(usize::MAX);
    if count > buf.len() / 4 {
        return Err(Error::Unexpected("a DNS config vector is longer than its data"));
    }
    Ok(count)
}

fn rule(buf: &mut &[u8]) -> Result<AccessPointRule> {
    if u32::deserialize(buf)? != ACCESS_POINT_RULE {
        return Err(Error::Unexpected("the DNS config has an unknown access point rule"));
    }
    let phone_prefix_rules = String::deserialize(buf)?;
    let dc_id = i32::deserialize(buf)?;
    let ips = (0..count(buf)?).map(|_| ip_port(buf)).collect::<Result<_>>()?;
    Ok(AccessPointRule { phone_prefix_rules, dc_id, ips })
}

fn ip_port(buf: &mut &[u8]) -> Result<IpPort> {
    let id = u32::deserialize(buf)?;
    if id != IP_PORT && id != IP_PORT_SECRET {
        return Err(Error::Unexpected("the DNS config has an unknown address type"));
    }
    // The address goes out in network order, so read as a little-endian int its bytes come lowest first.
    let ip = Ipv4Addr::from(i32::deserialize(buf)?.to_le_bytes());
    let port = u16::try_from(i32::deserialize(buf)?).map_err(|_| Error::Unexpected("a DNS config port is invalid"))?;
    let secret = if id == IP_PORT_SECRET { Some(Vec::<u8>::deserialize(buf)?) } else { None };
    Ok(IpPort { addr: SocketAddrV4::new(ip, port), secret })
}

/// The TXT strings of `name` from a DNS-over-HTTPS JSON endpoint.
async fn resolve_txt(host: &str, path: &str, name: &str) -> Result<Vec<String>> {
    let tcp = TcpStream::connect((host, 443)).await?;
    let mut stream = tls::connect(tcp, host).await?;
    let request = format!(
        "GET {path}?name={name}&type=TXT HTTP/1.1\r\nHost: {host}\r\nAccept: application/dns-json\r\n\
         Connection: close\r\n\r\n"
    );
    stream.write_all(request.as_bytes()).await?;
    // TLS holds a write back until it's flushed.
    stream.flush().await?;
    let (head, rest) = http1::read_head(&mut stream).await?;
    if head.status != 200 {
        return Err(Error::Unexpected("the DNS resolver didn't answer 200 OK"));
    }
    let body = read_body(&mut stream, &head, rest).await?;
    let body = std::str::from_utf8(&body).map_err(|_| Error::Unexpected("the DNS answer isn't text"))?;
    Ok(txt_records(body))
}

/// The body after `head`: `Content-Length` bytes, or everything up to the close, de-chunked if it came chunked.
async fn read_body<S: AsyncRead + Unpin>(stream: &mut S, head: &Head, mut body: Vec<u8>) -> io::Result<Vec<u8>> {
    let length = match head.header("content-length") {
        Some(value) => Some(value.parse::<usize>().map_err(|_| invalid("a bad Content-Length"))?),
        None => None,
    };
    if length.is_some_and(|n| n > MAX_BODY) {
        return Err(invalid("the DNS answer is too long"));
    }
    let mut chunk = [0; 4096];
    while length.is_none_or(|n| body.len() < n) {
        let n = match stream.read(&mut chunk).await {
            Ok(0) => break,
            Ok(n) => n,
            // Servers often close without a TLS close_notify; the read up to that point is whole.
            Err(e) if e.kind() == io::ErrorKind::UnexpectedEof && length.is_none() => break,
            Err(e) => return Err(e),
        };
        body.extend_from_slice(&chunk[..n]);
        if body.len() > MAX_BODY {
            return Err(invalid("the DNS answer is too long"));
        }
    }
    if let Some(n) = length {
        if body.len() < n {
            return Err(io::Error::new(io::ErrorKind::UnexpectedEof, "the DNS answer ended early"));
        }
        body.truncate(n);
    }
    let chunked = head.header("transfer-encoding").is_some_and(|v| v.to_ascii_lowercase().contains("chunked"));
    if chunked { dechunk(&body) } else { Ok(body) }
}

fn dechunk(mut data: &[u8]) -> io::Result<Vec<u8>> {
    let mut out = Vec::new();
    loop {
        let end = data.windows(2).position(|w| w == b"\r\n").ok_or_else(|| invalid("a chunk size line has no end"))?;
        let line = std::str::from_utf8(&data[..end]).map_err(|_| invalid("a chunk size isn't text"))?;
        let size = line.split(';').next().unwrap_or_default().trim();
        let size = usize::from_str_radix(size, 16).map_err(|_| invalid("a chunk size isn't hex"))?;
        data = &data[end + 2..];
        if size == 0 {
            return Ok(out);
        }
        out.extend_from_slice(data.get(..size).ok_or_else(|| invalid("a chunk is cut short"))?);
        data = data.get(size + 2..).ok_or_else(|| invalid("a chunk is cut short"))?;
    }
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

/// Every `"data"` string of the `"Answer"` array of a DNS-over-HTTPS JSON answer, without the quotes DNS keeps
/// around TXT strings.
fn txt_records(json: &str) -> Vec<String> {
    let Some(start) = json.find("\"Answer\"") else { return Vec::new() };
    let mut rest = &json[start..];
    let mut records = Vec::new();
    while let Some(at) = rest.find("\"data\"") {
        rest = rest[at + 6..].trim_start();
        let Some(value) = rest.strip_prefix(':').map(str::trim_start).and_then(|r| r.strip_prefix('"')) else {
            continue;
        };
        let Some((data, after)) = json_string(value) else { break };
        let data = data.strip_prefix('"').and_then(|d| d.strip_suffix('"')).unwrap_or(&data);
        records.push(data.to_owned());
        rest = after;
    }
    records
}

/// A JSON string's contents up to its closing quote, unescaped, and what follows it.
fn json_string(s: &str) -> Option<(String, &str)> {
    let mut out = String::new();
    let mut chars = s.char_indices();
    while let Some((i, c)) = chars.next() {
        match c {
            '"' => return Some((out, &s[i + 1..])),
            '\\' => {
                let escaped = match chars.next()?.1 {
                    'n' => '\n',
                    't' => '\t',
                    'r' => '\r',
                    'b' => '\u{8}',
                    'f' => '\u{c}',
                    'u' => {
                        let hex: String = (0..4).filter_map(|_| chars.next().map(|(_, c)| c)).collect();
                        char::from_u32(u32::from_str_radix(&hex, 16).ok()?).unwrap_or(char::REPLACEMENT_CHARACTER)
                    }
                    other => other,
                };
                out.push(escaped);
            }
            c => out.push(c),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use std::net::{SocketAddr, SocketAddrV4};

    use super::{AccessPointRule, DnsConfig, IpPort, cbc_decrypt, dechunk, txt_records};
    use crate::errors::Error;

    // A config Telegram published in 2019, as its two TXT records.
    const SHORT: &str =
        "LcmEoukF2bVjKwz3E+J9BsDdL+rv9lGqLQWIGXrWACT2ESk5xuOpA6Cz6klKRbhbwSiHOd2zC5PiR57j/OJHPpj4i+tw==";
    const LONG: &str = concat!(
        "umjjLFLpOKtPeW9zHLq2ypbMzg/zkqvPhvhr0bxrLZlgPQ04l2GpO/4qZgAx3tk3BDHbY6/gmG1e8eaFBq3YSqR5SZ5hQ1Cm5f4/",
        "o67GYcPJClaf1TiHq3wVfsQ5OLnyJRw9A2ZfUfzIXxoSklPJrVdF/4hM1ZdUE0eWDAbmYf7JCeao8ecVVwKndd4CZHZS9wyf1T7DIUh95VpQ",
        "sn2klLPA6gA/2YNXOh9gITvjZrKuXLwwh9hBHhPvxv",
    );

    #[test]
    fn reads_a_published_config() {
        let config = DnsConfig::parse(&[SHORT, LONG]).unwrap();
        assert_eq!((config.date(), config.expires()), (1562949126, 1565541126));
        let [rule] = config.rules() else { panic!("{:?}", config.rules()) };
        assert_eq!((rule.dc_id, rule.phone_prefix_rules.as_str()), (2, ""));
        let [IpPort { addr, secret: Some(secret) }] = rule.ips.as_slice() else { panic!("{:?}", rule.ips) };
        assert_eq!(*addr, "98.210.59.139:14544".parse::<SocketAddrV4>().unwrap());
        assert_eq!((secret[0], &secret[17..]), (0xee, &b"www.google.com"[..]));
        // That address needs its secret, so a plain connection has nowhere to go.
        assert!(config.plain_addresses(2, 0).is_empty());
    }

    #[test]
    fn expired_config_is_ignored() {
        let ip = IpPort { addr: "10.0.0.9:443".parse().unwrap(), secret: None };
        let rule = AccessPointRule { phone_prefix_rules: String::new(), dc_id: 2, ips: vec![ip] };
        let config = DnsConfig { date: 0, expires: 1000, rules: vec![rule] };
        assert_eq!(config.plain_addresses(2, 1000), ["10.0.0.9:443".parse::<SocketAddr>().unwrap()]);
        assert!(config.plain_addresses(2, 1001).is_empty(), "expired");
        assert!(config.plain_addresses(4, 0).is_empty(), "another DC");
    }

    #[test]
    fn rejects_a_changed_hash_bad_base64_and_a_wrong_length() {
        let changed = format!("M{}", &SHORT[1..]);
        assert!(matches!(DnsConfig::parse(&[changed.as_str(), LONG]), Err(Error::Security(_))));
        // Characters outside base64 are dropped, which leaves the wrong length.
        let bad = format!("#{}", &SHORT[1..]);
        assert!(matches!(DnsConfig::parse(&[bad.as_str(), LONG]), Err(Error::Unexpected(_))));
        assert!(matches!(DnsConfig::parse(&[&SHORT[1..], LONG]), Err(Error::Unexpected(_))));
        assert!(DnsConfig::parse::<&str>(&[]).is_err());
        let padding_inside = format!("{}=A", &SHORT[..SHORT.len() - 2]);
        assert!(DnsConfig::parse(&[padding_inside.as_str(), LONG]).is_err());
    }

    #[test]
    fn cbc_matches_nist_sp800_38a_f26() {
        let hex = |s: &str| (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect();
        let key: Vec<u8> = hex("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4");
        let mut data: Vec<u8> = hex(concat!(
            "f58c4c04d6e5f1ba779eabfb5f7bfbd69cfc4e967edb808d679f777bc6702c7d",
            "39f23369a9d9bacfa530e26304231461b2eb05e2c39be9fcda6c19078c6a9d1b",
        ));
        let iv = core::array::from_fn(|i| i as u8);
        cbc_decrypt(&mut data, &key.try_into().unwrap(), iv).unwrap();
        let plain: Vec<u8> = hex(concat!(
            "6bc1bee22e409f96e93d7e117393172aae2d8a571e03ac9c9eb76fac45af8e51",
            "30c81c46a35ce411e5fbc1191a0a52eff69f2445df4f9b17ad2b417be66c3710",
        ));
        assert_eq!(data, plain);
    }

    #[test]
    fn doh_json_parts_longer_first() {
        // The records come in any order, and DNS quotes each TXT string.
        let json = format!(
            r#"{{"Status":0,"Question":[{{"name":"apv3.stel.com.","type":16}}],"Answer":[{{"name":"apv3.stel.com.","type":16,"TTL":300,"data":"\"{SHORT}\""}},{{"name":"apv3.stel.com.","type":16,"TTL":300,"data": "{LONG}"}}]}}"#
        );
        let txt = txt_records(&json);
        assert_eq!(txt, [SHORT, LONG]);
        assert_eq!(DnsConfig::parse(&txt).unwrap().date(), 1562949126);
        assert!(txt_records(r#"{"Status":3,"Question":[{"name":"x","type":16}]}"#).is_empty());
    }

    #[test]
    fn dechunks_a_chunked_body() {
        assert_eq!(dechunk(b"4\r\nab\"c\r\n3;x=y\r\nd\\e\r\n0\r\n\r\n").unwrap(), b"ab\"cd\\e");
        assert!(dechunk(b"9\r\nshort\r\n").is_err());
        assert!(dechunk(b"zz\r\n").is_err());
    }
}
