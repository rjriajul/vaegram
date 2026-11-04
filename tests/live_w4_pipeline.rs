// Run with `cargo test --test live_w4_pipeline -- --ignored`; see tests/support/mod.rs for the env.
#[allow(dead_code)]
mod support;

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use vaegram::session::{RateCategory, RateLimits};
use vaegram::{Client, ClientBuilder, Error};

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("set {key}"))
}

fn user_builder() -> Option<ClientBuilder> {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return None;
    };
    Some(Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH")))
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn message_limit_paces_three_sends() {
    let Some(builder) = user_builder() else { return };
    let limits = RateLimits::default().limit(RateCategory::Message, 1.0, 1.0);
    let client = builder.rate_limits(limits).connect().await.unwrap();

    let start = Instant::now();
    let mut sent = Vec::new();
    for text in ["one", "two", "three"] {
        sent.push(client.send_message("me", text).await.unwrap().id);
    }
    let elapsed = start.elapsed();
    client.delete_messages("me", &sent).await.unwrap();
    // The first goes at once; the other two wait for a token each.
    assert!(elapsed >= Duration::from_secs(2), "three sends took {elapsed:?}");
    assert!(client.rate_limiter().unwrap().congestion() > 0.0);
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn no_updates_client_gets_me_and_no_updates() {
    let Some(builder) = user_builder() else { return };
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = seen.clone();
    let client =
        builder.no_updates().observer(move |call| log.lock().unwrap().push(call.name)).connect().await.unwrap();

    client.get_me().await.unwrap();
    // Calls after the opening one are named for the wrapper they go out in.
    let wrapped = seen.lock().unwrap().iter().filter(|name| **name == "invokeWithoutUpdates").count();
    assert!(wrapped >= 1, "{:?}", seen.lock().unwrap());
    // Without updates there's nothing for `run` to do.
    assert!(matches!(client.run().await, Err(Error::InvalidArgument(_))));
}
