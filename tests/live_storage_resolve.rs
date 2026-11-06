#![cfg(feature = "unstable-raw")]

#[allow(dead_code)]
mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use vaegram::Client;
use vaegram::raw::{Identifiable, functions};

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("set {key}"))
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn a_cached_username_is_resolved_once() {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return;
    };
    let (api_id, api_hash) = (env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    let name = "target/vaegram-live-storage-resolve";
    std::fs::copy(format!("{session}.session"), format!("{name}.session")).unwrap();

    let lookups = Arc::new(AtomicUsize::new(0));
    let counted = lookups.clone();
    let resolve_id = functions::contacts::ResolveUsername::CONSTRUCTOR_ID.to_le_bytes();
    let user = Client::builder(name, api_id, api_hash)
        .observer(move |call| {
            if call.request.windows(4).any(|w| w == resolve_id) {
                counted.fetch_add(1, Ordering::Relaxed);
            }
        })
        .connect()
        .await
        .unwrap();
    let id = user.resolve_username("BotFather").await.unwrap();
    let first = user.resolve_peer("@BotFather").await.unwrap();
    let again = user.resolve_peer("@botfather").await.unwrap();
    drop(user);
    std::fs::remove_file(format!("{name}.session")).unwrap();
    assert_eq!(first, again);
    assert!(id > 0);
    assert_eq!(lookups.load(Ordering::Relaxed), 1, "only resolve_username asked Telegram");
}
