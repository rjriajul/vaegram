#![cfg(feature = "websocket")]

#[cfg(feature = "unstable-raw")]
use std::sync::Arc;
#[cfg(feature = "unstable-raw")]
use std::time::Duration;

use vaegram::Client;
#[cfg(feature = "unstable-raw")]
use vaegram::connection::Options;
#[cfg(feature = "unstable-raw")]
use vaegram::raw::{base, functions};
#[cfg(feature = "unstable-raw")]
use vaegram::session::{Dc, Session};
use vaegram::types::Upload;

#[cfg(feature = "unstable-raw")]
fn websocket() -> Options {
    Options::new().websocket(true)
}

#[cfg(feature = "unstable-raw")]
#[tokio::test]
#[ignore = "connects to Telegram over WebSocket"]
async fn websocket_gets_config() {
    let dc = Dc::production(2).unwrap();
    let (session, _updates) = Session::connect(dc, None, &websocket(), Arc::default(), None).await.unwrap();
    let base::Pong::Pong(pong) = session.invoke(&functions::Ping { ping_id: 3 }).await.unwrap();
    assert_eq!(pong.ping_id, 3);
}

#[cfg(feature = "unstable-raw")]
#[tokio::test]
#[ignore = "connects to Telegram over WebSocket and waits five minutes"]
async fn websocket_survives_5_min_idle() {
    let dc = Dc::production(4).unwrap();
    let (session, _updates) = Session::connect(dc, None, &websocket(), Arc::default(), None).await.unwrap();
    tokio::time::sleep(Duration::from_secs(5 * 60)).await;
    let base::Pong::Pong(pong) = session.invoke(&functions::Ping { ping_id: 5 }).await.unwrap();
    assert_eq!(pong.ping_id, 5);
}

#[tokio::test]
#[ignore = "uses a logged-in user session; uploads and downloads a file in Saved Messages over WebSocket"]
async fn websocket_downloads_saved_file() {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let user = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .websocket()
        .connect()
        .await
        .unwrap();
    let data: Vec<u8> = (0..3_000_000u32).map(|i| (i * 7 + i / 251) as u8).collect();
    let sent = user.send_document("me", Upload::bytes("vaegram-ws.bin", &data)).await.unwrap();
    let downloaded = sent.download().await;
    sent.delete().await.unwrap();
    assert!(downloaded.unwrap() == data);
}
