mod support;

use vaegram::filters;
use vaegram::types::{InputMedia, Upload};

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot"]
async fn message_helpers_reply_read_and_refetch() {
    let Some(kit) = support::Kit::from_env().await else { return };
    kit.bot.on_message(filters::private() & filters::text(), |m| async move {
        if m.text == "album" {
            let album = [
                InputMedia::document(Upload::bytes("a.txt", b"a")),
                InputMedia::document(Upload::bytes("b.txt", b"b")),
            ];
            m.reply_media_group(album).await?;
        }
        Ok(())
    });

    let album = kit.user.send_and_wait_reply(&kit.bot, "album").await.unwrap();
    assert!(album.media_group_id.is_some());
    let replied = album.reply_to_message().await.unwrap().expect("the album replies to the request");
    assert_eq!(replied.text, "album");
    album.mark_as_read().await.unwrap();
    let again = album.refetch().await.unwrap().expect("the album item still exists");
    assert_eq!((again.id, again.chat.id), (album.id, album.chat.id));
}
