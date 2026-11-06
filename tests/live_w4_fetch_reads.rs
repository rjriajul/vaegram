use std::future::IntoFuture;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use vaegram::Client;
use vaegram::types::PeerRef;

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn history_replies_come_prefetched() {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION");
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let (api_id, api_hash): (i32, String) = (env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    let fetches = Arc::new(AtomicUsize::new(0));
    let counted = fetches.clone();
    let user = Client::builder(session, api_id, api_hash)
        .fetch_replies(true)
        .observer(move |call| {
            if call.name == "messages.getMessages" {
                counted.fetch_add(1, Ordering::Relaxed);
            }
        })
        .connect()
        .await
        .unwrap();

    let first = user.send_message(PeerRef::Me, "first").into_future().await.unwrap();
    let second = user.send_message(PeerRef::Me, "second").reply_to(first.id).into_future().await.unwrap();
    let before = fetches.load(Ordering::Relaxed);
    let mut history = user.get_chat_history(PeerRef::Me).limit(2);
    let mut replies = Vec::new();
    while let Some(m) = history.next().await.unwrap() {
        replies.push(m.reply_to_message().await.unwrap().map(|r| r.text().to_owned()));
    }
    let asked = fetches.load(Ordering::Relaxed) - before;
    user.delete_messages(PeerRef::Me, &[first.id, second.id]).await.unwrap();
    // One getMessages for the page; the replies were in hand when the history returned them.
    assert_eq!(asked, 1);
    assert_eq!(replies, [Some("first".to_owned()), None]);
}
