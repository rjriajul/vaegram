use std::io::Cursor;

use vaegram::types::{InputMedia, Upload};
use vaegram::{Client, Error};

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
async fn user_sends_one_upload_twice_and_in_an_album() {
    let Some(client) = user_client().await else { return };
    let data = vec![7; 600 * 1024];
    let file = client.upload_stream(Cursor::new(&data), data.len() as u64, "a.bin").await.unwrap();
    let other = client.upload_file(Upload::bytes("b.txt", b"hello")).await.unwrap();
    let mut sent = Vec::new();
    for _ in 0..2 {
        let message = client.send_document("me", &file).await.unwrap();
        let document = message.document().expect("no document");
        assert_eq!((document.file_name.as_deref(), document.file_size), (Some("a.bin"), data.len() as i64));
        sent.push(message.id);
    }
    let album = client.send_media_group("me", [InputMedia::document(&file), InputMedia::document(&other)]).await;
    let album = album.unwrap();
    sent.extend(album.iter().map(|m| m.id));
    client.delete_messages("me", &sent).await.unwrap();
    assert_eq!(album.len(), 2);
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn stop_transmission_ends_a_public_upload() {
    let Some(client) = user_client().await else { return };
    let data = vec![0; 20 << 20];
    let stopper = client.clone();
    let uploaded = client
        .upload_file(Upload::bytes("big.bin", &data))
        .progress(move |done, _| {
            if done > 0 {
                stopper.stop_transmission();
            }
        })
        .await;
    assert!(matches!(uploaded, Err(Error::Disconnected)), "{uploaded:?}");
    client.get_me().await.unwrap();
}
