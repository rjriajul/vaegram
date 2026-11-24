#[allow(dead_code)]
mod support;

use std::time::Duration;

use vaegram::enums::ChatAction;
use vaegram::types::{Message, Upload};

const CHUNK: usize = 512 * 1024;

/// A ~1.2 MB document the user sends to Saved Messages, and its bytes.
async fn sent_document(kit: &support::Kit) -> (Message, Vec<u8>) {
    let data: Vec<u8> = (0..1_200_000_u32).map(|i| (i % 251) as u8).collect();
    let message = kit.user.client.send_document("me", Upload::bytes("range.bin", &data)).await.unwrap();
    (message, data)
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn a_byte_range_matches_the_whole_file() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let (message, data) = sent_document(&kit).await;
    let whole = message.download().await;
    let range = message.download().offset(CHUNK as u64 + 5).limit(1000).await;
    kit.user.client.delete_messages("me", &[message.id]).await.unwrap();
    assert_eq!(whole.unwrap(), data);
    assert_eq!(range.unwrap(), data[CHUNK + 5..CHUNK + 1005]);
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn stream_media_equals_await() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let (message, data) = sent_document(&kit).await;
    let document = message.document().expect("no document");
    let mut stream = kit.user.client.stream_media(document);
    let mut streamed = Vec::new();
    let failed = loop {
        match stream.next().await {
            Ok(Some(chunk)) => streamed.extend(chunk),
            Ok(None) => break None,
            Err(e) => break Some(e),
        }
    };
    let awaited = kit.user.client.download_media(document).await;
    kit.user.client.delete_messages("me", &[message.id]).await.unwrap();
    assert!(failed.is_none(), "{failed:?}");
    assert_eq!(streamed, data);
    assert_eq!(awaited.unwrap(), streamed);
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn typing_repeats_then_stops() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let me = kit.user.client.get_me().await.unwrap();
    let handle = kit.bot.send_chat_action(me.id, ChatAction::Typing).repeat().await.unwrap();
    tokio::time::sleep(Duration::from_secs(9)).await;
    handle.stop().await.unwrap();
    kit.bot.send_chat_action(me.id, ChatAction::Cancel).await.unwrap();
}
