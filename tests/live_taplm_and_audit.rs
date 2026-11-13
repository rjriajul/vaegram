// Run with `cargo test --test live_taplm_and_audit -- --ignored`; see tests/support/mod.rs for the env.

#[allow(dead_code)]
mod support;

use std::time::Duration;

use tokio::sync::mpsc;
use vaegram::enums::FileType;
use vaegram::filters;
use vaegram::types::{
    ButtonStyle, FileSource, InlineKeyboardButton, InlineKeyboardMarkup, InputRichMessage, KeyboardButton,
    ReplyKeyboardMarkup, ReplyMarkup, RichBlock, RichBlockKind, RichButton, Text,
};
use vaegram::{Client, Error};

const WAIT: Duration = Duration::from_secs(30);

async fn custom_emoji_id(client: &Client) -> i64 {
    if let Ok(id) = std::env::var("VAEGRAM_CUSTOM_EMOJI_ID") {
        return id.parse().expect("VAEGRAM_CUSTOM_EMOJI_ID must be an integer document id");
    }
    let icons = client.get_forum_topic_icon_stickers().await.unwrap();
    let sticker = icons.first().expect("get_forum_topic_icon_stickers returned no stickers");
    sticker.custom_emoji_id.unwrap_or(sticker.id)
}

async fn bot_id_for_user(kit: &support::Kit) -> i64 {
    let me = kit.bot.get_me().await.unwrap();
    let username = me.username.expect("the kit bot has no username");
    kit.user.client.resolve_username(&username).await.unwrap()
}

/// TapLM open check 1: custom emoji in button text (`icon_custom_emoji_id` on inline & reply keyboard buttons) sent by
/// a bot. Set `VAEGRAM_CUSTOM_EMOJI_ID` to test a specific non-free emoji; otherwise uses a free forum-topic icon
/// emoji. No human or second account needed.
#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn bot_custom_emoji_in_button_text() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = &kit.user.client;
    let user_id = user.get_me().await.unwrap().id;
    let bot_id = bot_id_for_user(&kit).await;
    let emoji_id = custom_emoji_id(&kit.bot).await;

    let (tx, mut rx) = mpsc::unbounded_channel();
    user.on_message(filters::private() & filters::user(bot_id), move |m| {
        let _ = tx.send(m);
        async { Ok(()) }
    });

    let inline_markup = InlineKeyboardMarkup::new([vec![
        InlineKeyboardButton::callback("Tap", "ce_btn").style(ButtonStyle::Primary).icon_custom_emoji_id(emoji_id),
    ]]);
    kit.bot
        .send_message(user_id, "button icon check")
        .reply_markup(inline_markup)
        .await
        .unwrap_or_else(|e| panic!("bot sending inline button with icon_custom_emoji_id({emoji_id}) failed: {e:?}"));
    let user_inline = tokio::time::timeout(WAIT, rx.recv()).await.unwrap().unwrap();
    let Some(ReplyMarkup::Inline(read_inline)) = user_inline.reply_markup() else {
        panic!("expected inline reply markup on received message");
    };
    assert_eq!(read_inline.rows[0][0].icon_custom_emoji_id, Some(emoji_id));

    let reply_markup = ReplyKeyboardMarkup::new([vec![
        KeyboardButton::new("Reply").icon_custom_emoji_id(emoji_id),
    ]])
    .one_time(true);
    kit.bot
        .send_message(user_id, "reply icon check")
        .reply_markup(reply_markup)
        .await
        .unwrap_or_else(|e| panic!("bot sending reply button with icon_custom_emoji_id({emoji_id}) failed: {e:?}"));
    let user_reply = tokio::time::timeout(WAIT, rx.recv()).await.unwrap().unwrap();
    let Some(ReplyMarkup::Keyboard(read_reply)) = user_reply.reply_markup() else {
        panic!("expected reply keyboard markup on received message");
    };
    assert_eq!(read_reply.rows[0][0].icon_custom_emoji_id, Some(emoji_id));

    support::cleanup(
        user.delete_messages(bot_id, &[user_inline.id, user_reply.id]).await,
        "deleting custom-emoji button messages",
    );
}

/// TapLM open check 2: rich messages for a bot (blocks, HTML, Markdown, editing into rich text, rich buttons, and
/// custom emoji inside rich messages, plus verifying `get_rich_message` is user-only). No human or second account
/// needed.
#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn bot_rich_messages_and_custom_emoji() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = &kit.user.client;
    let user_id = user.get_me().await.unwrap().id;
    let bot_id = bot_id_for_user(&kit).await;
    let emoji_id = custom_emoji_id(&kit.bot).await;

    let (tx, mut rx) = mpsc::unbounded_channel();
    user.on_message(filters::private() & filters::user(bot_id), move |m| {
        let _ = tx.send(m);
        async { Ok(()) }
    });

    // 1. Blocks with custom emoji + RichBlock::buttons
    let button = RichButton::new(&InlineKeyboardButton::callback("RichBtn", "rb")).unwrap();
    let blocks = [
        RichBlock::paragraph(Text::new().plain("emoji: ").custom_emoji("👍", emoji_id)),
        RichBlock::buttons([button]),
    ];
    let sent_blocks = kit.bot.send_rich_message(user_id, InputRichMessage::blocks(blocks)).await.unwrap();
    assert!(sent_blocks.rich_message.is_some());
    let user_blocks = tokio::time::timeout(WAIT, rx.recv()).await.unwrap().unwrap();
    let fetched = user.get_rich_message(bot_id, user_blocks.id).await.unwrap().expect("rich message exists");
    let rich = fetched.rich_message.expect("rich_message parsed");
    assert!(!rich.is_partial);
    let read_blocks = rich.blocks();
    assert!(
        matches!(&read_blocks[0].kind(), RichBlockKind::Paragraph(t) if t.text().contains("👍")),
        "unexpected paragraph block: {:?}",
        read_blocks[0].kind()
    );
    assert!(
        matches!(&read_blocks[1].kind(), RichBlockKind::Buttons { buttons, .. } if buttons.len() == 1),
        "unexpected buttons block: {:?}",
        read_blocks[1].kind()
    );

    // Bots cannot call messages.getRichMessage.
    let bot_fetch_err = kit.bot.get_rich_message(user_id, sent_blocks.id).await.unwrap_err();
    assert!(bot_fetch_err.is("BOT_METHOD_INVALID"), "expected BOT_METHOD_INVALID, got {bot_fetch_err:?}");

    // 2. HTML with <tg-emoji> and Markdown, then edit into rich blocks.
    let html_src = format!("<p>html <tg-emoji emoji-id=\"{emoji_id}\">👍</tg-emoji></p>");
    let sent_html = kit.bot.send_rich_message(user_id, InputRichMessage::html(html_src)).await.unwrap();
    assert!(sent_html.rich_message.is_some());
    let user_html = tokio::time::timeout(WAIT, rx.recv()).await.unwrap().unwrap();
    let fetched_html = user.get_rich_message(bot_id, user_html.id).await.unwrap().unwrap();
    assert!(fetched_html.rich_message.is_some());

    let md_src = format!("**md** ![👍](tg://emoji?id={emoji_id})");
    let sent_md = kit.bot.send_rich_message(user_id, InputRichMessage::markdown(md_src)).await.unwrap();
    let user_md = tokio::time::timeout(WAIT, rx.recv()).await.unwrap().unwrap();
    let edited = sent_md.edit_text(InputRichMessage::blocks([RichBlock::paragraph("edited rich")])).await.unwrap();
    assert!(edited.rich_message.is_some());

    support::cleanup(
        user.delete_messages(bot_id, &[user_blocks.id, user_html.id, user_md.id]).await,
        "deleting rich message test messages",
    );
}

/// Checks the `missing_features.md` §3.2 `copy_message` `NoMessage` claim against live Telegram: creates a temporary
/// broadcast channel, copies a message with `copy_message` and `copy_messages`, asserts both return message ids,
/// and deletes the channel. No human or second account needed.
#[tokio::test]
#[ignore = "creates and deletes a broadcast channel with a logged-in user session"]
async fn copy_message_to_broadcast_channel_returns_message() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = kit.user.client.clone();
    let channel = match user.create_channel("vaegram copy check", "deleted right away").await {
        Ok(channel) => channel,
        Err(e) if support::flood_limited(&e, "creating a broadcast channel") => return,
        Err(e) => panic!("create_channel failed: {e:?}"),
    };

    let steps = tokio::spawn({
        let user = user.clone();
        let channel_id = channel.id;
        async move {
            let src = user.send_message("me", "copy to broadcast").await.unwrap();
            let copied = user.copy_message(channel_id, "me", src.id).await.unwrap();
            assert_eq!(copied.chat.id, channel_id);
            assert_eq!(copied.text(), "copy to broadcast");
            assert!(copied.id > 0);

            let batch = user.copy_messages(channel_id, "me", &[src.id]).await.unwrap();
            assert_eq!(batch.len(), 1);
            assert!(batch[0] > 0);

            support::cleanup(user.delete_messages("me", &[src.id]).await, "deleting copy source message");
        }
    })
    .await;

    support::cleanup(user.delete_channel(channel.id).await, "deleting temporary broadcast channel");
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}

/// Checks the `missing_features.md` §3.2 thumbnail `file_id` claim against live Telegram:
/// 1) Document/sticker thumbnail `file_id`s (`FileType::Thumbnail`, which encode `volume_id: 0, local_id: 0`) can be
///    downloaded with `download_media`, while `FileSource::file_id` refuses re-sending them as download-only.
/// 2) Optionally (if `VAEGRAM_HTTP_BOT_API_FILE_ID` is set) checks that an HTTP Bot API `file_id` from another bot
///    token cannot be reused directly.
/// No human or second account needed.
#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn thumbnail_file_ids_download_and_validate() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let icons = kit.bot.get_forum_topic_icon_stickers().await.unwrap();
    let Some(thumb) = icons.iter().find_map(|s| s.thumbs.first()) else {
        eprintln!("skipped: no forum topic icon sticker carried a thumbnail");
        return;
    };

    assert_eq!(FileType::of(&thumb.file_id).unwrap(), FileType::Thumbnail);
    assert!(matches!(FileSource::file_id(&thumb.file_id), Err(Error::InvalidArgument(_))));

    let bytes = kit.bot.download_media(&thumb.file_id).await.unwrap();
    assert!(!bytes.is_empty(), "downloaded thumbnail was empty");

    if let Ok(external_id) = std::env::var("VAEGRAM_HTTP_BOT_API_FILE_ID") {
        let err = kit.bot.download_media(external_id.as_str()).await.unwrap_err();
        assert!(err.rpc().is_some() || matches!(err, Error::InvalidArgument(_)), "unexpected result: {err:?}");
    }
}
