#[path = "../benches/support/mockdc.rs"]
mod mockdc;

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use mockdc::{GROUP_ID, MockDc, get_file};
use vaegram::raw::{base, functions};
use vaegram::{Client, filters};

#[tokio::test(flavor = "multi_thread")]
async fn full_client_talks_to_the_mock() {
    let mock = MockDc::start(Duration::ZERO);
    let client = Client::builder("mock", 1, "mock").session_string(mock.session_string()).connect().await.unwrap();

    let base::Pong::Pong(pong) = client.invoke(&functions::Ping { ping_id: 42 }).await.unwrap();
    assert_eq!(pong.ping_id, 42);

    let sent = client.send_message(-GROUP_ID, "hi").await.unwrap();
    assert_eq!(sent.text, "hi");
    assert_eq!(mock.stats.sent.load(Ordering::Relaxed), 1);

    let get = get_file(0, 4096);
    let base::upload::File::File(file) = client.invoke(&get).await.unwrap() else { panic!("expected upload.file") };
    assert_eq!(file.bytes.len(), 4096);
    assert_eq!(file.bytes[300], 300u32 as u8);

    let seen = Arc::new(AtomicU64::new(0));
    let s = seen.clone();
    client.on_message(filters::all(), move |m| {
        assert_eq!(m.text, "ping");
        s.fetch_add(1, Ordering::Relaxed);
        async { Ok(()) }
    });
    let states = mock.stats.states.load(Ordering::Relaxed);
    let runner = tokio::spawn({
        let client = client.clone();
        async move { client.run().await }
    });
    while mock.stats.states.load(Ordering::Relaxed) == states {
        tokio::time::sleep(Duration::from_millis(1)).await;
    }
    tokio::time::sleep(Duration::from_millis(20)).await;
    mock.push(3);
    tokio::time::timeout(Duration::from_secs(5), async {
        while seen.load(Ordering::Relaxed) < 3 {
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("pushed messages didn't reach the handler");
    runner.abort();
}
