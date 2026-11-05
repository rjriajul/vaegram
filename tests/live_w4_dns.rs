#![cfg(feature = "dns-config")]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use vaegram::connection::{DnsConfig, Options};
use vaegram::raw::{base, functions};
use vaegram::session::Session;
use vaegram::session::internals::data_center::Dc;

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

#[tokio::test]
#[ignore = "asks public DNS-over-HTTPS resolvers"]
async fn dns_config_from_both_resolvers() {
    for test in [false, true] {
        let config = DnsConfig::fetch(test).await.unwrap();
        assert!(i64::from(config.date()) <= now() && now() <= i64::from(config.expires()), "{config:?}");
        assert!(config.rules().iter().any(|r| !r.ips.is_empty()), "{config:?}");
    }
}

#[tokio::test]
#[ignore = "asks public DNS-over-HTTPS resolvers and connects to Telegram"]
async fn dns_address_accepts_a_handshake() {
    let config = DnsConfig::fetch(false).await.unwrap();
    let mut plain = config.rules().iter().flat_map(|r| r.ips.iter().map(move |ip| (r.dc_id, ip)));
    let Some((id, ip)) = plain.find(|(_, ip)| ip.secret.is_none()) else {
        eprintln!("skipped: every address in today's config needs a secret: {config:?}");
        return;
    };
    let dc = Dc { id, addr: SocketAddr::V4(ip.addr), test: false };
    let (session, _updates) = Session::connect(dc, None, &Options::default(), Arc::default(), None).await.unwrap();
    let base::Pong::Pong(pong) = session.invoke(&functions::Ping { ping_id: 9 }).await.unwrap();
    assert_eq!(pong.ping_id, 9);
}
