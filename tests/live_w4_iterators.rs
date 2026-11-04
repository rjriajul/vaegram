// Run with `cargo test --test live_w4_iterators -- --ignored`; see tests/support/mod.rs for the env.
#[allow(dead_code)]
mod support;

use vaegram::Client;

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("set {key}"))
}

async fn user() -> Option<Client> {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return None;
    };
    let builder = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    Some(builder.connect().await.unwrap())
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn dialog_total_equals_get_dialogs_count() {
    let Some(client) = user().await else { return };
    let mut dialogs = client.get_dialogs();
    let before = dialogs.total().await.unwrap();
    assert_eq!(before, client.get_dialogs_count(false).await.unwrap());

    let mut dialogs = client.get_dialogs().limit(5);
    while dialogs.next().await.unwrap().is_some() {}
    assert_eq!(dialogs.total().await.unwrap(), before);
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn history_total_and_ranges_agree_with_the_chat() {
    let Some(client) = user().await else { return };
    let sent = [
        client.send_message("me", "one").await.unwrap().id,
        client.send_message("me", "two").await.unwrap().id,
        client.send_message("me", "three").await.unwrap().id,
    ];
    let (first, last) = (sent[0], sent[2]);

    let mut newest_first = client.get_chat_history("me").min_id(first).max_id(last);
    let mut ids = Vec::new();
    while let Some(m) = newest_first.next().await.unwrap() {
        ids.push(m.id);
    }
    let mut oldest_first = client.get_chat_history("me").min_id(first).max_id(last).reverse();
    let mut reversed = Vec::new();
    while let Some(m) = oldest_first.next().await.unwrap() {
        reversed.push(m.id);
    }
    let total = client.get_chat_history("me").total().await.unwrap();
    let counted = client.get_chat_history_count("me").await.unwrap();
    client.delete_messages("me", &sent).await.unwrap();

    assert_eq!(ids, [sent[2], sent[1], sent[0]]);
    assert_eq!(reversed, sent);
    assert_eq!(total, counted);
}
