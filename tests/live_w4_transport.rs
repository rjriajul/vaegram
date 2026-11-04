// Run with `cargo test --test live_w4_transport -- --ignored`; see tests/support/mod.rs for the env.
#[allow(dead_code)]
mod support;

use vaegram::Client;
use vaegram::connection::Transport;
use vaegram::raw::base;

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("set {key}"))
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn full_transport_gets_me() {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return;
    };
    let client = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .transport(Transport::Full)
        .connect()
        .await
        .unwrap();
    assert!(matches!(client.get_me().await.unwrap(), base::User::User(_)));
}
