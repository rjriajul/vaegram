use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;
use std::{fmt, io};

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::raw::{base, types};

#[derive(Clone)]
pub enum Proxy {
    Socks5 { host: String, port: u16, auth: Option<(String, String)> },
    MtProxy { host: String, port: u16, secret: Secret },
}

impl Proxy {
    // Lets Telegram show the proxy's sponsored channel.
    pub(crate) fn input_client_proxy(&self) -> Option<base::InputClientProxy> {
        match self {
            Proxy::MtProxy { host, port, .. } => {
                let proxy = types::InputClientProxy { address: host.clone(), port: (*port).into() };
                Some(base::InputClientProxy::InputClientProxy(Box::new(proxy)))
            }
            Proxy::Socks5 { .. } => None,
        }
    }
}

// Credentials and secrets stay out of logs.
impl fmt::Debug for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (kind, host, port) = match self {
            Proxy::Socks5 { host, port, .. } => ("Socks5", host, port),
            Proxy::MtProxy { host, port, .. } => ("MtProxy", host, port),
        };
        f.debug_struct(kind).field("host", host).field("port", port).finish_non_exhaustive()
    }
}

/// An MTProxy secret in hex. Plain and `dd` secrets are supported, fake-TLS `ee` ones are not.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Secret {
    pub(crate) key: [u8; 16],
    pub(crate) padded: bool,
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Secret").field("padded", &self.padded).finish_non_exhaustive()
    }
}

impl FromStr for Secret {
    type Err = io::Error;

    fn from_str(hex: &str) -> io::Result<Self> {
        let bytes: Option<Vec<u8>> =
            (0..hex.len()).step_by(2).map(|i| hex.get(i..i + 2).and_then(|h| u8::from_str_radix(h, 16).ok())).collect();
        let invalid = |msg| io::Error::new(io::ErrorKind::InvalidInput, msg);
        match bytes.as_deref() {
            Some(key) if key.len() == 16 => Ok(Secret { key: key.try_into().unwrap(), padded: false }),
            Some([0xdd, key @ ..]) if key.len() == 16 => Ok(Secret { key: key.try_into().unwrap(), padded: true }),
            Some([0xee, ..]) => {
                Err(invalid("fake-TLS (ee) MTProxy secrets are not supported; use a plain or dd secret"))
            }
            _ => Err(invalid("MTProxy secret must be 32 hex digits, optionally prefixed with dd")),
        }
    }
}

// https://www.rfc-editor.org/rfc/rfc1928 and rfc1929 for the username/password step.
pub(crate) async fn socks5<S: AsyncRead + AsyncWrite + Unpin>(
    s: &mut S,
    target: SocketAddr,
    auth: Option<&(String, String)>,
) -> io::Result<()> {
    let methods: &[u8] = if auth.is_some() { &[5, 2, 0, 2] } else { &[5, 1, 0] };
    s.write_all(methods).await?;
    let mut reply = [0u8; 2];
    s.read_exact(&mut reply).await?;
    match (reply, auth) {
        ([5, 0], _) => {}
        ([5, 2], Some((user, pass))) => {
            if user.len() > 255 || pass.len() > 255 {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "SOCKS5 credentials are limited to 255 bytes"));
            }
            let mut msg = Vec::with_capacity(3 + user.len() + pass.len());
            msg.extend_from_slice(&[1, user.len() as u8]);
            msg.extend_from_slice(user.as_bytes());
            msg.push(pass.len() as u8);
            msg.extend_from_slice(pass.as_bytes());
            s.write_all(&msg).await?;
            s.read_exact(&mut reply).await?;
            if reply != [1, 0] {
                return Err(io::Error::other("SOCKS5 proxy rejected the username or password"));
            }
        }
        _ => return Err(io::Error::other("SOCKS5 proxy accepted none of the offered auth methods")),
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
        return Err(io::Error::other(format!("SOCKS5 proxy could not connect to {target} (reply {})", head[1])));
    }
    let bound_len = match head[3] {
        1 => 4,
        4 => 16,
        3 => s.read_u8().await? as usize,
        _ => return Err(io::Error::new(io::ErrorKind::InvalidData, "SOCKS5 proxy sent an unknown address type")),
    };
    let mut bound = [0u8; 255 + 2];
    s.read_exact(&mut bound[..bound_len + 2]).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Proxy, Secret, SocketAddr, socks5};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn parses_secrets() {
        let plain: Secret = "00112233445566778899aabbccddeeff".parse().unwrap();
        let padded: Secret = "dd00112233445566778899aabbccddeeff".parse().unwrap();
        assert_eq!((plain.key[15], plain.padded, padded.key, padded.padded), (0xff, false, plain.key, true));
        let fake_tls = "ee00112233445566778899aabbccddeeff676f6f676c652e636f6d";
        for bad in [fake_tls, "0011", "zz112233445566778899aabbccddeeff", "dd0"] {
            assert!(bad.parse::<Secret>().is_err(), "{bad}");
        }
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
