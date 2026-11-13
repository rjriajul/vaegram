use vaegram::Client;

async fn user_client() -> Option<Client> {
    let (Ok(session), Ok(api_id)) = (std::env::var("VAEGRAM_USER_SESSION"), std::env::var("VAEGRAM_API_ID")) else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION, VAEGRAM_API_ID and VAEGRAM_API_HASH");
        return None;
    };
    let api_hash = std::env::var("VAEGRAM_API_HASH").expect("set VAEGRAM_API_HASH");
    Some(Client::builder(session, api_id.parse().unwrap(), api_hash).connect().await.unwrap())
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn get_users_reads_me() {
    let Some(client) = user_client().await else { return };
    let me = client.get_users(["me"]).await.unwrap().remove(0).expect("Telegram knows me");
    assert!(me.is_self);
    assert!(me.phone_number.is_some());
    if !me.usernames.is_empty() {
        assert!(me.usernames.iter().any(|u| Some(&u.username) == me.username.as_ref()));
    }
    if let Some(photo) = &me.photo {
        assert!(!client.download_media(&photo.small_file_id()).await.unwrap().is_empty());
    }
}
