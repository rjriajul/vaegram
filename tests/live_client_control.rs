#[allow(dead_code)]
mod support;

use std::time::Duration;

use tokio::sync::mpsc;
use vaegram::Client;

const WAIT: Duration = Duration::from_secs(30);

async fn user_client() -> Option<Client> {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return None;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let client = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    Some(client.connect().await.unwrap())
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn hooks_bracket_run() {
    let Some(client) = user_client().await else { return };
    let (tx, mut seen) = mpsc::unbounded_channel();
    let started = tx.clone();
    client.on_start(move |_| {
        let _ = started.send("start");
        async { Ok(()) }
    });
    client.on_stop(move |_| {
        let _ = tx.send("stop");
        async { Ok(()) }
    });
    let run = tokio::spawn({
        let client = client.clone();
        async move { client.run().await }
    });
    assert_eq!(tokio::time::timeout(WAIT, seen.recv()).await.unwrap(), Some("start"));
    client.stop();
    assert!(run.await.unwrap().is_ok());
    assert_eq!(seen.recv().await, Some("stop"));
}
