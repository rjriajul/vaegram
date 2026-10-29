use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::{fmt, io};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::fake_tls;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::utils::{base64_decode, base64_encode};

#[derive(Clone)]
#[non_exhaustive]
pub enum Proxy {
    Socks5 {
        host: String,
        port: u16,
        auth: Option<(String, String)>,
    },
    /// IPv4 data centers only.
    Socks4 {
        host: String,
        port: u16,
    },
    /// HTTP CONNECT. The proxy is spoken to in plain HTTP, not HTTPS, so `auth` (Basic, the only kind) crosses
    /// the network readable.
    Http {
        host: String,
        port: u16,
        auth: Option<(String, String)>,
    },
    MtProxy {
        host: String,
        port: u16,
        secret: Secret,
    },
}

impl Proxy {
    // Lets Telegram show the proxy's sponsored channel.
    pub(crate) fn input_client_proxy(&self) -> Option<base::InputClientProxy> {
        match self {
            Proxy::MtProxy { host, port, .. } => {
                let proxy = types::InputClientProxy { address: host.clone(), port: (*port).into() };
                Some(base::InputClientProxy::InputClientProxy(Box::new(proxy)))
            }
            Proxy::Socks5 { .. } | Proxy::Socks4 { .. } | Proxy::Http { .. } => None,
        }
    }

    pub(crate) fn endpoint(&self) -> (&str, u16) {
        match self {
            Proxy::Socks5 { host, port, .. }
            | Proxy::Socks4 { host, port }
            | Proxy::Http { host, port, .. }
            | Proxy::MtProxy { host, port, .. } => (host, *port),
        }
    }

    /// An MTProxy needs no request here: it reads the DC from the obfuscation header.
    pub(crate) async fn handshake<S: AsyncRead + AsyncWrite + Unpin>(
        &self,
        s: &mut S,
        target: SocketAddr,
    ) -> Result<()> {
        match self {
            Proxy::Socks5 { auth, .. } => socks5(s, target, auth.as_ref()).await,
            Proxy::Socks4 { .. } => socks4(s, target).await,
            Proxy::Http { auth, .. } => http_connect(s, target, auth.as_ref()).await,
            Proxy::MtProxy { secret, .. } => match &secret.domain {
                Some(domain) => Ok(fake_tls::handshake(s, &secret.key, domain).await?),
                None => Ok(()),
            },
        }
    }
}

// Credentials and secrets stay out of logs.
impl fmt::Debug for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let kind = match self {
            Proxy::Socks5 { .. } => "Socks5",
            Proxy::Socks4 { .. } => "Socks4",
            Proxy::Http { .. } => "Http",
            Proxy::MtProxy { .. } => "MtProxy",
        };
        let (host, port) = self.endpoint();
        f.debug_struct(kind).field("host", &host).field("port", &port).finish_non_exhaustive()
    }
}

/// An MTProxy secret in hex or base64, as in `t.me/proxy` links: plain, `dd` (padded) or `ee` (fake TLS).
#[derive(Clone, PartialEq, Eq)]
pub struct Secret {
    pub(crate) key: [u8; 16],
    pub(crate) padded: bool,
    pub(crate) domain: Option<Box<[u8]>>,
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let tls = self.domain.is_some();
        f.debug_struct("Secret").field("padded", &self.padded).field("fake_tls", &tls).finish_non_exhaustive()
    }
}

impl FromStr for Secret {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        let hex: Option<Vec<u8>> =
            (0..s.len()).step_by(2).map(|i| s.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok())).collect();
        let invalid = Error::InvalidArgument;
        let bytes = hex.or_else(|| base64_decode(s)).ok_or(invalid("MTProxy secret is not hex or base64"))?;
        let key = |k: &[u8]| -> [u8; 16] { k[..16].try_into().unwrap() };
        match bytes.as_slice() {
            k if k.len() == 16 => Ok(Secret { key: key(k), padded: false, domain: None }),
            [0xdd, k @ ..] if k.len() == 16 => Ok(Secret { key: key(k), padded: true, domain: None }),
            [0xee, k @ ..] if k.len() > 16 && k.len() <= 16 + fake_tls::MAX_DOMAIN => {
                Ok(Secret { key: key(k), padded: true, domain: Some(k[16..].into()) })
            }
            _ => Err(invalid("MTProxy secret must be 16 bytes, optionally prefixed with dd, or ee with a domain")),
        }
    }
}

// https://www.rfc-editor.org/rfc/rfc1928 and rfc1929 for the username/password step.
pub(crate) async fn socks5<S: AsyncRead + AsyncWrite + Unpin>(
    s: &mut S,
    target: SocketAddr,
    auth: Option<&(String, String)>,
) -> Result<()> {
    if auth.is_some_and(|(user, pass)| user.len() > 255 || pass.len() > 255) {
        return Err(Error::InvalidArgument("SOCKS5 credentials are limited to 255 bytes"));
    }
    let methods: &[u8] = if auth.is_some() { &[5, 2, 0, 2] } else { &[5, 1, 0] };
    s.write_all(methods).await?;
    let mut reply = [0u8; 2];
    s.read_exact(&mut reply).await?;
    match (reply, auth) {
        ([5, 0], _) => {}
        ([5, 2], Some((user, pass))) => {
            let mut msg = Vec::with_capacity(3 + user.len() + pass.len());
            msg.extend_from_slice(&[1, user.len() as u8]);
            msg.extend_from_slice(user.as_bytes());
            msg.push(pass.len() as u8);
            msg.extend_from_slice(pass.as_bytes());
            s.write_all(&msg).await?;
            s.read_exact(&mut reply).await?;
            if reply != [1, 0] {
                return Err(io::Error::other("SOCKS5 proxy rejected the username or password").into());
            }
        }
        _ => return Err(io::Error::other("SOCKS5 proxy accepted none of the offered auth methods").into()),
    }

    let mut request = Vec::with_capacity(22);
    request.extend_from_slice(&[5, 1, 0]);
    match target.ip() {
        IpAddr::V4(ip) => {
            request.push(1);
            request.extend_from_slice(&ip.octets());
        }
        IpAddr::V6(ip) => {
            request.push(4);
            request.extend_from_slice(&ip.octets());
        }
    }
    request.extend_from_slice(&target.port().to_be_bytes());
    s.write_all(&request).await?;

    let mut head = [0u8; 4];
    s.read_exact(&mut head).await?;
    if head[..2] != [5, 0] {
        return Err(io::Error::other(format!("SOCKS5 proxy could not connect to {target} (reply {})", head[1])).into());
    }
    let bound_len = match head[3] {
        1 => 4,
        4 => 16,
        3 => s.read_u8().await? as usize,
        _ => {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "SOCKS5 proxy sent an unknown address type").into());
        }
    };
    let mut bound = [0u8; 255 + 2];
    s.read_exact(&mut bound[..bound_len + 2]).await?;
    Ok(())
}

// https://www.openssh.com/txt/socks4.protocol, with an empty user id.
pub(crate) async fn socks4<S: AsyncRead + AsyncWrite + Unpin>(s: &mut S, target: SocketAddr) -> Result<()> {
    let IpAddr::V4(ip) = target.ip() else {
        return Err(Error::InvalidArgument("SOCKS4 cannot reach IPv6 addresses"));
    };
    let mut request = [4, 1, 0, 0, 0, 0, 0, 0, 0];
    request[2..4].copy_from_slice(&target.port().to_be_bytes());
    request[4..8].copy_from_slice(&ip.octets());
    s.write_all(&request).await?;
    let mut reply = [0u8; 8];
    s.read_exact(&mut reply).await?;
    // The reply's version byte is 0; anything else isn't a SOCKS4 proxy.
    if reply[0] != 0 || reply[1] != 0x5a {
        return Err(
            io::Error::other(format!("SOCKS4 proxy could not connect to {target} (reply {:?})", &reply[..2])).into()
        );
    }
    Ok(())
}

// https://www.rfc-editor.org/rfc/rfc9110#name-connect
pub(crate) async fn http_connect<S: AsyncRead + AsyncWrite + Unpin>(
    s: &mut S,
    target: SocketAddr,
    auth: Option<&(String, String)>,
) -> Result<()> {
    let mut request = format!("CONNECT {target} HTTP/1.1\r\nHost: {target}\r\n");
    if let Some((user, pass)) = auth {
        // https://www.rfc-editor.org/rfc/rfc7617#section-2: the user-id can't hold a colon.
        if user.contains(':') {
            return Err(Error::InvalidArgument("an HTTP proxy user name can't contain ':'"));
        }
        let token = base64_encode(format!("{user}:{pass}").as_bytes());
        request.push_str(&format!("Proxy-Authorization: Basic {token}\r\n"));
    }
    request.push_str("\r\n");
    s.write_all(request.as_bytes()).await?;

    // Byte by byte, so nothing after the headers is swallowed.
    let mut head = Vec::with_capacity(128);
    while !head.ends_with(b"\r\n\r\n") {
        if head.len() == 8192 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "HTTP proxy reply headers are too long").into());
        }
        head.push(s.read_u8().await?);
    }
    let line = head.split(|&b| b == b'\r').next().unwrap_or_default();
    let status = line.strip_prefix(b"HTTP/1.").and_then(|rest| rest.split(|&b| b == b' ').nth(1));
    if !status.is_some_and(|s| s.len() == 3 && s[0] == b'2') {
        // The proxy's own words, cut short and printable only: they end up in logs.
        let printable = |c: &char| c.is_ascii_graphic() || *c == ' ';
        let line: String = line.iter().take(64).map(|&b| char::from(b)).filter(printable).collect();
        return Err(io::Error::other(format!("HTTP proxy could not connect to {target}: {line}")).into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Error, Proxy, Secret, SocketAddr, http_connect, socks4, socks5};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn parses_secrets() {
        let plain: Secret = "00112233445566778899aabbccddeeff".parse().unwrap();
        let padded: Secret = "dd00112233445566778899aabbccddeeff".parse().unwrap();
        assert_eq!(padded.key, plain.key);
        assert!(padded.padded);
        let fake_tls: Secret = "ee000102030405060708090a0b0c0d0e0f676f6f676c652e636f6d".parse().unwrap();
        assert_eq!(fake_tls.key, std::array::from_fn(|i| i as u8));
        assert!(fake_tls.padded);
        assert_eq!(fake_tls.domain.as_deref(), Some(&b"google.com"[..]));
        assert_eq!("7gABAgMEBQYHCAkKCwwNDg9nb29nbGUuY29t".parse::<Secret>().unwrap(), fake_tls);
        let no_domain = "ee000102030405060708090a0b0c0d0e0f";
        for bad in [no_domain, "0011", "zz112233445566778899aabbccddeeff", "dd0", "!!"] {
            assert!(matches!(bad.parse::<Secret>(), Err(Error::InvalidArgument(_))), "{bad}");
        }
    }

    #[tokio::test]
    async fn http_connect_with_basic_auth() {
        let (mut client, mut server) = tokio::io::duplex(512);
        let auth = ("user".to_string(), "pw".to_string());
        let target: SocketAddr = "149.154.167.51:443".parse().unwrap();
        let proxy = async move {
            let expected = "CONNECT 149.154.167.51:443 HTTP/1.1\r\nHost: 149.154.167.51:443\r\n\
                            Proxy-Authorization: Basic dXNlcjpwdw==\r\n\r\n";
            let mut buf = vec![0u8; expected.len()];
            server.read_exact(&mut buf).await.unwrap();
            assert_eq!(buf, expected.as_bytes());
            server.write_all(b"HTTP/1.1 200 Connection established\r\nVia: x\r\n\r\nnext").await.unwrap();
            server
        };
        let (result, _server) = tokio::join!(http_connect(&mut client, target, Some(&auth)), proxy);
        result.unwrap();
        let mut next = [0u8; 4];
        client.read_exact(&mut next).await.unwrap();
        assert_eq!(&next, b"next");

        let (mut client, mut server) = tokio::io::duplex(512);
        server.write_all(b"HTTP/1.1 407 Proxy Authentication Required\r\n\r\n").await.unwrap();
        let err = http_connect(&mut client, target, None).await.unwrap_err();
        assert!(err.to_string().contains("407"), "{err}");
    }

    #[tokio::test]
    async fn http_connect_reads_the_status_from_the_status_line_only() {
        let target: SocketAddr = "149.154.167.51:443".parse().unwrap();
        for reply in [&b"HTTP/1.1\r\nX: 200 y\r\n\r\n"[..], b"SSH-2.0 200 ok\r\n\r\n"] {
            let (mut client, mut server) = tokio::io::duplex(512);
            server.write_all(reply).await.unwrap();
            assert!(http_connect(&mut client, target, None).await.is_err(), "{reply:?} was taken for a 200");
        }

        let (mut client, mut server) = tokio::io::duplex(512);
        server.write_all(b"HTTP/1.1 403 \x1b[2Jgo away\r\n\r\n").await.unwrap();
        let err = http_connect(&mut client, target, None).await.unwrap_err().to_string();
        assert!(err.ends_with("HTTP/1.1 403 [2Jgo away"), "{err}");
    }

    #[tokio::test]
    async fn socks4_request_and_reply() {
        let (mut client, mut server) = tokio::io::duplex(64);
        let target: SocketAddr = "149.154.167.51:443".parse().unwrap();
        let proxy = async move {
            let mut buf = [0u8; 9];
            server.read_exact(&mut buf).await.unwrap();
            assert_eq!(buf, [4, 1, 1, 187, 149, 154, 167, 51, 0]);
            server.write_all(&[0, 0x5a, 0, 0, 0, 0, 0, 0]).await.unwrap();
            server
        };
        let (result, _server) = tokio::join!(socks4(&mut client, target), proxy);
        result.unwrap();
        let ipv6 = socks4(&mut client, "[2001:db8::1]:443".parse().unwrap()).await;
        assert!(matches!(ipv6, Err(Error::InvalidArgument(_))), "{ipv6:?}");

        let (mut client, mut server) = tokio::io::duplex(64);
        server.write_all(&[4, 0x5a, 0, 0, 0, 0, 0, 0]).await.unwrap();
        assert!(matches!(socks4(&mut client, target).await, Err(Error::Io(_))), "a reply with version 4 passed");
    }

    #[tokio::test]
    async fn caller_mistakes_are_invalid_arguments() {
        let target: SocketAddr = "149.154.167.51:443".parse().unwrap();
        let (mut client, _server) = tokio::io::duplex(64);
        let colon = ("a:b".to_string(), "pw".to_string());
        let http = http_connect(&mut client, target, Some(&colon)).await;
        assert!(matches!(http, Err(Error::InvalidArgument(_))), "{http:?}");
        let long = ("u".repeat(256), "pw".to_string());
        let socks = socks5(&mut client, target, Some(&long)).await;
        assert!(matches!(socks, Err(Error::InvalidArgument(_))), "{socks:?}");
    }

    #[test]
    fn debug_hides_credentials() {
        let socks = Proxy::Socks5 { host: "h".into(), port: 1, auth: Some(("user".into(), "hunter2".into())) };
        let secret = "00112233445566778899aabbccddeeff".parse().unwrap();
        let mtproxy = Proxy::MtProxy { host: "h".into(), port: 1, secret };
        for shown in [format!("{socks:?}"), format!("{mtproxy:?}")] {
            assert!(!shown.contains("hunter2") && !shown.contains("user") && !shown.contains("255"), "{shown}");
        }
    }

    #[tokio::test]
    async fn socks5_with_password_and_domain_reply() {
        let (mut client, mut server) = tokio::io::duplex(256);
        let auth = ("user".into(), "pw".into());
        let target: SocketAddr = "149.154.167.51:443".parse().unwrap();
        let proxy = async move {
            let mut buf = [0u8; 64];
            server.read_exact(&mut buf[..4]).await.unwrap();
            assert_eq!(buf[..4], [5, 2, 0, 2]);
            server.write_all(&[5, 2]).await.unwrap();
            server.read_exact(&mut buf[..9]).await.unwrap();
            assert_eq!(&buf[..9], b"\x01\x04user\x02pw");
            server.write_all(&[1, 0]).await.unwrap();
            server.read_exact(&mut buf[..10]).await.unwrap();
            assert_eq!(buf[..10], [5, 1, 0, 1, 149, 154, 167, 51, 1, 187]);
            server.write_all(b"\x05\x00\x00\x03\x05proxy\x01\xbb").await.unwrap();
            server
        };
        let (result, mut server) = tokio::join!(socks5(&mut client, target, Some(&auth)), proxy);
        result.unwrap();
        server.write_all(b"next").await.unwrap();
        let mut next = [0u8; 4];
        client.read_exact(&mut next).await.unwrap();
        assert_eq!(&next, b"next");
    }

    #[tokio::test]
    async fn socks5_surfaces_connect_failure() {
        let (mut client, mut server) = tokio::io::duplex(256);
        let target: SocketAddr = "[2001:db8::1]:443".parse().unwrap();
        let proxy = async {
            let mut buf = [0u8; 22];
            server.read_exact(&mut buf[..3]).await.unwrap();
            server.write_all(&[5, 0]).await.unwrap();
            server.read_exact(&mut buf).await.unwrap();
            assert_eq!(buf[3], 4);
            server.write_all(&[5, 5, 0, 1, 0, 0, 0, 0, 0, 0]).await.unwrap();
        };
        let (result, ()) = tokio::join!(socks5(&mut client, target, None), proxy);
        assert!(result.is_err());
    }
}
