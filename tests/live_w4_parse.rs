// Run with `cargo test --test live_w4_parse -- --ignored`; see tests/support/mod.rs for the env.
#[allow(dead_code)]
mod support;

use vaegram::Client;
use vaegram::enums::{MessageEntityType, ParseMode};

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("set {key}"))
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn default_mode_sends_bold_and_italic() {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return;
    };
    let client = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .connect()
        .await
        .unwrap();
    let sent = client.send_message("me", "**bold** <i>italic</i> 1 &lt; 2").parse_mode(ParseMode::Default).await;
    let sent = sent.unwrap();
    client.delete_messages("me", &[sent.id]).await.unwrap();

    assert_eq!(sent.text(), "bold italic 1 < 2");
    let entities = sent.entities().unwrap_or_default();
    let has = |kind: MessageEntityType| entities.iter().any(|e| e.kind == kind);
    assert!(has(MessageEntityType::Bold) && has(MessageEntityType::Italic), "{entities:?}");
}
