use vaegram::Client;

async fn user_client() -> Option<Client> {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return None;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let (api_id, api_hash) = (env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    Some(Client::builder(session, api_id, api_hash).connect().await.unwrap())
}

async fn my_id(client: &Client) -> i64 {
    let me = client.get_me().await.unwrap();
    me.id
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn get_chat_fills_full_info_for_the_test_chat() {
    let Some(client) = user_client().await else { return };
    let Ok(chat_id) = std::env::var("VAEGRAM_CHAT_ID") else {
        eprintln!("skipped: set VAEGRAM_CHAT_ID");
        return;
    };
    let chat = client.get_chat(chat_id.parse::<i64>().unwrap()).await.unwrap();
    let info = chat.full_info.expect("get_chat fills full_info");
    assert!(chat.title.is_some());
    assert!(chat.members_count.is_some_and(|n| n > 0));
    if let Some(linked) = &info.linked_chat {
        assert!(linked.id < 0);
    }
    if let Some(photo) = &chat.photo {
        assert!(!client.download_media(&photo.small_file_id()).await.unwrap().is_empty());
    }
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn get_chat_reads_my_pinned_message_id() {
    let Some(client) = user_client().await else { return };
    let me = my_id(&client).await;
    let sent = client.send_message(me, "pinned by the chat info test").await.unwrap();
    client.pin_chat_message(me, sent.id).await.unwrap();
    let pinned = client.get_chat(me).await.map(|chat| chat.full_info.and_then(|info| info.pinned_message_id));
    client.unpin_chat_message(me, sent.id).await.unwrap();
    client.delete_messages(me, &[sent.id]).await.unwrap();
    assert_eq!(pinned.unwrap(), Some(sent.id));
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn get_chat_reads_public_photo_when_set() {
    let Some(client) = user_client().await else { return };
    let me = my_id(&client).await;
    let info = client.get_chat(me).await.unwrap().full_info.expect("get_chat fills full_info");
    assert!(info.common_chats_count.is_some());
    match info.public_photo {
        Some(photo) => assert!(!client.download_media(&photo.big_file_id()).await.unwrap().is_empty()),
        None => eprintln!("live-unverified: this account has no public photo"),
    }
}
