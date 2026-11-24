// Run with `cargo test --test live_w4_filters -- --ignored`; see tests/support/mod.rs for the env.
#[allow(dead_code)]
mod support;

use std::time::Duration;

use tokio::sync::mpsc;
use vaegram::{Client, filters};

const WAIT: Duration = Duration::from_secs(30);

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("set {key}"))
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a chat it can pin in"]
async fn pinned_message_sees_a_pin() {
    let (Ok(session), Ok(chat)) = (std::env::var("VAEGRAM_USER_SESSION"), std::env::var("VAEGRAM_CHAT_ID")) else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION and VAEGRAM_CHAT_ID");
        return;
    };
    let chat: i64 = chat.parse().unwrap();
    let user = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .connect()
        .await
        .unwrap();
    let (tx, mut pins) = mpsc::unbounded_channel();
    user.on_message(filters::pinned_message() & filters::chat(chat), move |m| {
        let _ = tx.send(m.id);
        async { Ok(()) }
    });
    let running = tokio::spawn({
        let user = user.clone();
        async move { user.run().await }
    });

    let sent = user.send_message(chat, "pin me").await.unwrap();
    if let Err(e) = user.pin_chat_message(chat, sent.id).await {
        user.delete_messages(chat, &[sent.id]).await.unwrap();
        user.stop();
        let _ = running.await;
        assert!(support::flood_limited(&e, "pinning a message"), "{e:?}");
        return;
    }
    let seen = tokio::time::timeout(WAIT, pins.recv()).await;
    user.unpin_chat_message(chat, sent.id).await.unwrap();
    user.delete_messages(chat, &[sent.id]).await.unwrap();
    user.stop();
    let _ = running.await;
    assert!(matches!(seen, Ok(Some(_))), "the pin service message never reached the handler: {seen:?}");
}
