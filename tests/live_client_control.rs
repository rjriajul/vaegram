#[allow(dead_code)]
mod support;

use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use vaegram::raw::{base, functions};
use vaegram::types::Upload;
use vaegram::{Client, Error, filters};

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

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn disconnect_ends_run_and_requests() {
    let Some(client) = user_client().await else { return };
    let run = tokio::spawn({
        let client = client.clone();
        async move { client.run().await }
    });
    client.get_me().await.unwrap();
    client.disconnect();
    assert!(tokio::time::timeout(WAIT, run).await.unwrap().unwrap().is_ok());
    assert!(matches!(client.get_me().await, Err(Error::Disconnected)));
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn stop_transmission_cancels_an_upload() {
    let Some(client) = user_client().await else { return };
    let data = vec![0; 20 << 20];
    let stopper = client.clone();
    let sent = client
        .send_document("me", Upload::bytes("big.bin", &data))
        .progress(move |done, _| {
            if done > 0 {
                stopper.stop_transmission();
            }
        })
        .await;
    if let Ok(message) = &sent {
        client.delete_messages("me", &[message.id]).await.unwrap();
    }
    assert!(matches!(sent, Err(Error::Disconnected)), "{sent:?}");
    client.get_me().await.unwrap();
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn restart_fires_connect_hooks() {
    let Some(client) = user_client().await else { return };
    let (tx, mut seen) = mpsc::unbounded_channel();
    let down = tx.clone();
    client.on_connect(move |_| {
        let _ = tx.send("connect");
        async { Ok(()) }
    });
    client.on_disconnect(move |_| {
        let _ = down.send("disconnect");
        async { Ok(()) }
    });
    let run = tokio::spawn({
        let client = client.clone();
        async move { client.run().await }
    });
    client.restart().unwrap();
    let mut both = Vec::new();
    while both.len() < 2 {
        both.push(tokio::time::timeout(WAIT, seen.recv()).await.unwrap().unwrap());
    }
    both.sort_unstable();
    assert_eq!(both, ["connect", "disconnect"]);
    client.get_me().await.unwrap();
    client.stop();
    assert!(run.await.unwrap().is_ok());
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn invoke_in_dc_reaches_the_other_dc() {
    let Some(client) = user_client().await else { return };
    let base::NearestDc::NearestDc(home) = client.invoke(&functions::help::GetNearestDc {}).await.unwrap();
    let other = if home.this_dc == 2 { 4 } else { 2 };
    let base::NearestDc::NearestDc(there) =
        client.invoke_in_dc(other, &functions::help::GetNearestDc {}).await.unwrap();
    assert_eq!(there.this_dc, other);
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn bot_skips_missed_updates_without_catch_up() {
    let (Ok(session), Ok(token)) = (std::env::var("VAEGRAM_USER_SESSION"), std::env::var("VAEGRAM_BOT_TOKEN")) else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name) and VAEGRAM_BOT_TOKEN");
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let (api_id, api_hash): (i32, String) = (env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    let user = Client::builder(session, api_id, api_hash.clone()).connect().await.unwrap();
    let bot = |catch_up| {
        let builder = Client::builder("target/vaegram-catch-up-bot", api_id, api_hash.clone());
        builder.bot_token(token.clone()).catch_up(catch_up).start()
    };

    // A run that stops at once leaves the state saved.
    let first = bot(true).await.unwrap();
    first.stop();
    first.run().await.unwrap();
    let base::User::User(me) = first.get_me().await.unwrap() else { panic!("the bot has no user") };
    drop(first);
    let bot_id = user.resolve_username(&me.username.expect("the bot has no username")).await.unwrap();
    let missed = user.send_message(bot_id, "/missed").await.unwrap();

    let second = bot(false).await.unwrap();
    let (tx, mut texts) = mpsc::unbounded_channel();
    second.on_message(filters::private(), move |m| {
        let _ = tx.send(m.text);
        async { Ok(()) }
    });
    let (started, running) = oneshot::channel();
    let started = Mutex::new(Some(started));
    second.on_start(move |_| {
        let _ = started.lock().unwrap().take().map(|s| s.send(()));
        async { Ok(()) }
    });
    let run = tokio::spawn({
        let second = second.clone();
        async move { second.run().await }
    });
    tokio::time::timeout(WAIT, running).await.unwrap().unwrap();
    let live = user.send_message(bot_id, "/live").await.unwrap();
    let seen = tokio::time::timeout(WAIT, texts.recv()).await.unwrap();
    second.stop();
    assert!(run.await.unwrap().is_ok());
    user.delete_messages(bot_id, &[missed.id, live.id]).await.unwrap();
    assert_eq!(seen.as_deref(), Some("/live"));
}
