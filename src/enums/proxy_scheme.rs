/// The kind of a `Proxy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProxyScheme {
    Socks4,
    Socks5,
    Http,
    MtProxy,
}

impl ProxyScheme {
    pub fn as_str(self) -> &'static str {
        match self {
            ProxyScheme::Socks4 => "socks4",
            ProxyScheme::Socks5 => "socks5",
            ProxyScheme::Http => "http",
            ProxyScheme::MtProxy => "mtproxy",
        }
    }
}
