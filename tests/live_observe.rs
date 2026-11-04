#![cfg(feature = "metrics")]

use metrics_util::debugging::{DebugValue, DebuggingRecorder};
use vaegram::Client;

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn metrics_see_get_me() {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    // The observer runs on the connection's task, so only a global recorder sees it.
    let recorder = DebuggingRecorder::new();
    let snapshotter = recorder.snapshotter();
    recorder.install().unwrap();
    let client = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"))
        .observer(vaegram::observe::metrics)
        .connect()
        .await
        .unwrap();
    client.get_me().await.unwrap();

    let ok_get_users = snapshotter.snapshot().into_vec().into_iter().any(|(key, _, _, value)| {
        let key = key.key();
        let label = |name: &str| key.labels().find(|l| l.key() == name).map(|l| l.value().to_owned());
        key.name() == "vaegram_requests_total"
            && label("method").as_deref() == Some("users.getUsers")
            && label("outcome").as_deref() == Some("ok")
            && matches!(value, DebugValue::Counter(n) if n > 0)
    });
    assert!(ok_get_users, "no successful users.getUsers was counted");
}
