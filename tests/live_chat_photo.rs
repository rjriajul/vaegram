// Run with `cargo test --test live_chat_photo -- --ignored`; see tests/support/mod.rs for the env.
// Live behaviour is unverified: this test has not been run.

#[allow(dead_code)] // this binary drives only the user half of the kit
mod support;

#[tokio::test]
#[ignore]
async fn animated_photo_downloads() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let Some(chat_id) = std::env::var("VAEGRAM_CHAT_ID").ok().and_then(|v| v.parse::<i64>().ok()) else {
        eprintln!("skipped: set VAEGRAM_CHAT_ID");
        return;
    };
    let user = kit.user.client;
    let chat = user.get_chat(chat_id).await.unwrap();
    let Some(animation) = chat.photo.and_then(|p| p.animation) else {
        eprintln!("skipped: the chat photo isn't animated");
        return;
    };
    let bytes = user.download_media(animation.file_id.as_str()).await.unwrap();
    assert!(!bytes.is_empty());
}
