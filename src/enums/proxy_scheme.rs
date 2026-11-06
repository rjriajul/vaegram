/// The kind of a `Proxy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProxyScheme {
    /// A SOCKS4 proxy (`socks4://`, `socks4a://`): no credentials, IPv4 data centers only.
    Socks4,
    /// A SOCKS5 proxy (`socks5://`, `socks5h://`, `tg://socks`), with or without a username and password.
    Socks5,
    /// An HTTP proxy reached with `CONNECT` in plain HTTP (`http://`).
    Http,
    /// Telegram's MTProxy (`tg://proxy`), which takes a secret instead of credentials.
    MtProxy,
    /// An MTProxy reached through an HTTPS relay (`tg://webproxy`).
    Web,
}

impl ProxyScheme {
    /// The scheme's lowercase name: `socks4`, `socks5`, `http`, `mtproxy` or `web`.
    pub fn as_str(self) -> &'static str {
        match self {
            ProxyScheme::Socks4 => "socks4",
            ProxyScheme::Socks5 => "socks5",
            ProxyScheme::Http => "http",
            ProxyScheme::MtProxy => "mtproxy",
            ProxyScheme::Web => "web",
        }
    }
}
