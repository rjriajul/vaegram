#[allow(dead_code)]
mod support;

use vaegram::types::LinkPreviewOptions;

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn preview_options_round_trip() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let options = LinkPreviewOptions::new().url("https://telegram.org").prefer_large_media(true);
    let sent = kit.user.client.send_message("me", "link preview round trip").link_preview_options(options).await;
    let sent = sent.unwrap();
    let shown = sent.link_preview_options.clone();
    kit.user.client.delete_messages("me", &[sent.id]).await.unwrap();
    let shown = shown.expect("no link preview on the sent message");
    // The url is the web page's as Telegram stores it, which normalizes it to "https://telegram.org/".
    assert_eq!(shown.url.as_deref().map(|u| u.trim_end_matches('/')), Some("https://telegram.org"));
    assert!(shown.prefer_large_media);
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn send_web_page_shows_url() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let sent = kit.user.client.send_web_page("me", "look", "https://telegram.org").await.unwrap();
    let url = sent.link_preview_options.as_ref().and_then(|o| o.url.clone());
    kit.user.client.delete_messages("me", &[sent.id]).await.unwrap();
    assert_eq!(url.as_deref().map(|u| u.trim_end_matches('/')), Some("https://telegram.org"));
}
