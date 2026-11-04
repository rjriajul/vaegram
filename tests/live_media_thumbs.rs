// Run with `cargo test --test live_media_thumbs -- --ignored`; see tests/support/mod.rs for the env.

#[allow(dead_code)] // this binary drives only the bot half of the kit
mod support;

#[tokio::test]
#[ignore = "posts a photo to VAEGRAM_CHAT_ID: needs a bot token and VAEGRAM_THUMB_PHOTO (an image file)"]
async fn sent_document_has_thumbs() {
    let (Ok(chat_id), Ok(path)) = (std::env::var("VAEGRAM_CHAT_ID"), std::env::var("VAEGRAM_THUMB_PHOTO")) else {
        eprintln!("skipped: set VAEGRAM_CHAT_ID and VAEGRAM_THUMB_PHOTO (a large image file)");
        return;
    };
    let Some(kit) = support::Kit::from_env().await else { return };
    let chat_id: i64 = chat_id.parse().unwrap();
    let sent = kit.bot.send_photo(chat_id, path.as_str()).await.unwrap();
    let photo = sent.photo().expect("a photo came back");
    assert!(!photo.thumbs.is_empty(), "a large photo has smaller sizes");
    let bytes = kit.bot.download_media(&photo.thumbs[0].file_id).await.unwrap();
    assert!(!bytes.is_empty());
    kit.bot.delete_messages(chat_id, &[sent.id]).await.unwrap();
}
