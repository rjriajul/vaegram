use std::time::Duration;

use vaegram::Client;

/// With `run` going, `recover_gaps` asks Telegram for what was missed and answers; without `run` it refuses.
#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn recover_gaps_answers_while_run_is_going() {
    let vars = ["VAEGRAM_USER_SESSION", "VAEGRAM_API_ID", "VAEGRAM_API_HASH"].map(std::env::var);
    let [Ok(session), Ok(api_id), Ok(api_hash)] = vars else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION, VAEGRAM_API_ID and VAEGRAM_API_HASH");
        return;
    };
    let client = Client::builder(session, api_id.parse().unwrap(), api_hash).connect().await.unwrap();
    assert!(client.recover_gaps().await.is_err(), "recover_gaps worked before run");
    let runner = tokio::spawn({
        let client = client.clone();
        async move { client.run().await }
    });
    let mut recovered = None;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if let Ok(counts) = client.recover_gaps().chat_ids([0]).await {
            recovered = Some(counts);
            break;
        }
    }
    client.stop();
    runner.await.unwrap().unwrap();
    assert!(recovered.is_some(), "recover_gaps never got an answer from run");
}
