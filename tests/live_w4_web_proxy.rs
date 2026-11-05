#![cfg(feature = "web-proxy")]

use std::sync::Arc;

use vaegram::connection::{Options, Proxy};
use vaegram::raw::{base, functions};
use vaegram::session::Session;
use vaegram::session::internals::data_center;

#[tokio::test]
#[ignore = "connects to Telegram through a web proxy relay"]
async fn web_proxy_carries_a_handshake() {
    let Ok(link) = std::env::var("VAEGRAM_WEB_PROXY") else {
        eprintln!("skipped: set VAEGRAM_WEB_PROXY to a tg://webproxy link");
        return;
    };
    let proxy: Proxy = link.parse().unwrap();
    let options = Options { proxy: Some(proxy), ..Options::default() };
    let dc = data_center::find(&[], 2, false, false).unwrap();
    let (session, _updates) = Session::connect(dc, None, &options, Arc::default(), None).await.unwrap();
    let base::Pong::Pong(pong) = session.invoke(&functions::Ping { ping_id: 5 }).await.unwrap();
    assert_eq!(pong.ping_id, 5);
}
