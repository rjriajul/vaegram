mod support;

use vaegram::filters;
use vaegram::types::{InputMedia, Text, Upload};

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot"]
async fn message_helpers_format_reply_read_and_refetch() {
    let Some(kit) = support::Kit::from_env().await else { return };
    kit.bot.on_message(filters::private() & filters::text(), |m| async move {
        if m.text == "format" {
            m.reply(Text::new().bold("b").plain(" ").link("l", "https://t.me/")).await?;
        } else if m.text == "album" {
            let album = [
                InputMedia::document(Upload::bytes("a.txt", b"a")),
                InputMedia::document(Upload::bytes("b.txt", b"b")),
            ];
            m.reply_media_group(album).await?;
        }
        Ok(())
    });

    // Asked first, so a late album item can't be taken for this reply.
    let formatted = kit.user.send_and_wait_reply(&kit.bot, "format").await.unwrap();
    assert_eq!(formatted.html_text(), "<b>b</b> <a href=\"https://t.me/\">l</a>");
    assert_eq!(formatted.markdown_text(), "**b** [l](https://t.me/)");

    let album = kit.user.send_and_wait_reply(&kit.bot, "album").await.unwrap();
    assert!(album.media_group_id.is_some());
    let replied = album.reply_to_message().await.unwrap().expect("the album replies to the request");
    assert_eq!(replied.text, "album");
    album.mark_as_read().await.unwrap();
    let again = album.refetch().await.unwrap().expect("the album item still exists");
    assert_eq!((again.id, again.chat.id), (album.id, album.chat.id));
}
