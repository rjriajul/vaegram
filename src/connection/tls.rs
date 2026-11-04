use std::io;
use std::sync::{Arc, LazyLock};

use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

/// One configuration for every TLS connection: Mozilla's roots, TLS 1.2 and 1.3, HTTP/1.1.
static CONFIG: LazyLock<Arc<ClientConfig>> = LazyLock::new(|| {
    let roots = rustls::RootCertStore { roots: webpki_roots::TLS_SERVER_ROOTS.to_vec() };
    // The provider is named, so another crate turning on a second rustls backend can't make this ambiguous.
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("ring supports TLS 1.2 and 1.3")
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Arc::new(config)
});

/// TLS over `stream` to `host`, whose certificate must name it.
pub(crate) async fn connect<S: AsyncRead + AsyncWrite + Unpin>(stream: S, host: &str) -> io::Result<TlsStream<S>> {
    let name = ServerName::try_from(host.to_owned()).map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    TlsConnector::from(CONFIG.clone()).connect(name, stream).await
}

#[cfg(test)]
mod tests {
    use super::{CONFIG, connect};

    #[test]
    fn the_config_has_roots_and_asks_for_http1() {
        assert_eq!(CONFIG.alpn_protocols, [b"http/1.1".to_vec()]);
    }

    #[tokio::test]
    async fn a_bad_host_name_is_refused_before_any_byte() {
        let (ours, _theirs) = tokio::io::duplex(64);
        let refused = connect(ours, "not a host").await.err().map(|e| e.kind());
        assert_eq!(refused, Some(std::io::ErrorKind::InvalidInput));
    }
}
