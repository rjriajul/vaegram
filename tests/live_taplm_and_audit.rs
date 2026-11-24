// Run with `cargo test --test live_taplm_and_audit -- --ignored --test-threads=1`; see tests/support/mod.rs.
// These tests share session files, so run with `--test-threads=1`.

#[allow(dead_code)]
mod support;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::sync::mpsc;
use vaegram::enums::FileType;
use vaegram::filters;
use vaegram::raw::core::gzip_packed;
use vaegram::raw::{Identifiable, Serializable, base, functions, types};
use vaegram::storage::recording;
use vaegram::types::{
    ButtonStyle, FileSource, InlineKeyboardButton, InlineKeyboardMarkup, InputRichMessage, KeyboardButton,
    ReplyKeyboardMarkup, ReplyMarkup, RichBlock, RichBlockKind, RichButton, RichText, RichTextKind, Text,
};
use vaegram::{Client, Error};

const WAIT: Duration = Duration::from_secs(30);

async fn custom_emoji_with_alt(client: &Client) -> (i64, String) {
    let id = match std::env::var("VAEGRAM_CUSTOM_EMOJI_ID") {
        Ok(id) => id.parse().expect("VAEGRAM_CUSTOM_EMOJI_ID must be an integer document id"),
        Err(_) => {
            let icons = client.get_forum_topic_icon_stickers().await.unwrap();
            icons.first().expect("get_forum_topic_icon_stickers returned no stickers").id
        }
    };
    let stickers = client.get_custom_emoji_stickers(&[id]).await.unwrap();
    let alt = stickers.into_iter().next().and_then(|s| s.emoji).expect("custom emoji sticker has no alt emoji");
    (id, alt)
}

fn find_custom_emoji(text: &RichText) -> Option<(i64, String)> {
    match text.kind() {
        RichTextKind::CustomEmoji { id, alt, .. } => Some((id, alt)),
        RichTextKind::Concat(parts) => parts.iter().find_map(find_custom_emoji),
        RichTextKind::Bold(t)
        | RichTextKind::Italic(t)
        | RichTextKind::Underline(t)
        | RichTextKind::Strikethrough(t)
        | RichTextKind::Spoiler(t)
        | RichTextKind::Subscript(t)
        | RichTextKind::Superscript(t)
        | RichTextKind::Marked(t)
        | RichTextKind::Code(t)
        | RichTextKind::BankCard(t)
        | RichTextKind::TonAddress(t)
        | RichTextKind::Mention(t)
        | RichTextKind::Hashtag(t)
        | RichTextKind::Cashtag(t)
        | RichTextKind::BotCommand(t)
        | RichTextKind::Url { text: t, .. }
        | RichTextKind::AnchorLink { text: t, .. }
        | RichTextKind::Email { text: t, .. }
        | RichTextKind::Phone { text: t, .. }
        | RichTextKind::TextMention { text: t, .. }
        | RichTextKind::Reference { text: t, .. }
        | RichTextKind::DateTime { text: t, .. }
        | RichTextKind::Diff { text: t, .. }
        | RichTextKind::Button { text: t, .. } => find_custom_emoji(&t),
        RichTextKind::Empty | RichTextKind::Plain(_) | RichTextKind::Anchor { .. } | RichTextKind::Math(_) => None,
        _ => None,
    }
}

fn assert_rich_emoji_paragraph(block: &RichBlock, prefix: &str, emoji_id: i64, alt: &str, what: &str) {
    let RichBlockKind::Paragraph(t) = block.kind() else {
        panic!("{what}: expected paragraph block, got {:?}", block.kind());
    };
    if let Some((got_id, _got_alt)) = find_custom_emoji(&t) {
        assert_eq!(got_id, emoji_id, "{what}: custom emoji document id mismatch");
        assert!(t.text().starts_with(prefix), "{what}: expected prefix {prefix:?}, got {:?}", t.text());
    } else {
        assert_eq!(
            t.text(),
            format!("{prefix}{alt}"),
            "{what}: downgraded custom emoji must carry the emoji document's alt {alt:?}, not placeholder"
        );
        eprintln!(
            "note ({what}): Telegram downgraded CustomEmoji({emoji_id}) to plain alt {alt:?} \
             (requires bot owner Telegram Premium or a Fragment additional username)"
        );
    }
}

struct RemoveOnDrop(PathBuf);

impl Drop for RemoveOnDrop {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn unpack_if_gzip(body: &[u8]) -> std::borrow::Cow<'_, [u8]> {
    if !body.starts_with(&gzip_packed::ID.to_le_bytes()) {
        return std::borrow::Cow::Borrowed(body);
    }
    match gzip_packed::unpack(body) {
        Ok(plain) => std::borrow::Cow::Owned(plain),
        Err(_) => std::borrow::Cow::Borrowed(body),
    }
}

/// Checks whether the recording contains an outgoing `messages.sendMessage` whose body holds `button` bytes.
fn has_send_message_with(records: &[recording::Record], button: &[u8]) -> bool {
    let id_bytes = functions::messages::SendMessage::CONSTRUCTOR_ID.to_le_bytes();
    records.iter().filter(|r| r.outgoing).any(|rec| {
        let body = unpack_if_gzip(&rec.body);
        body.windows(4).any(|w| w == id_bytes) && body.windows(button.len()).any(|w| w == button)
    })
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
    let (Ok(session), Ok(token)) = (std::env::var("VAEGRAM_USER_SESSION"), std::env::var("VAEGRAM_BOT_TOKEN")) else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name) and VAEGRAM_BOT_TOKEN");
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let (api_id, api_hash): (i32, String) = (env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));

    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
    let rec_path = std::env::temp_dir().join(format!("vaegram-btn-icon-{}-{nonce}.rec", std::process::id()));
    let _ = std::fs::remove_file(&rec_path);
    let _rec_guard = RemoveOnDrop(rec_path.clone());

    let user = Client::builder(session, api_id, api_hash.clone()).catch_up(false).connect().await.unwrap();
    let bot_builder =
        Client::builder("target/vaegram-kit-bot", api_id, api_hash).bot_token(token).catch_up(false).record(&rec_path);
    let Some(bot) = support::start_bot(bot_builder).await else { return };

    let (started_tx, mut started) = mpsc::unbounded_channel();
    for client in [&user, &bot] {
        let started_tx = started_tx.clone();
        client.on_start(move |_| {
            let _ = started_tx.send(());
            async { Ok(()) }
        });
        let client = client.clone();
        tokio::spawn(async move { client.run().await });
    }
    for _ in 0..2 {
        tokio::time::timeout(WAIT, started.recv()).await.expect("a client's run didn't start");
    }
    let _disconnect = support::Disconnect(vec![user.clone(), bot.clone()]);

    let user_id = user.get_me().await.unwrap().id;
    let bot_me = bot.get_me().await.unwrap();
    let bot_username = bot_me.username.as_deref().expect("the kit bot has no username");
    let bot_id = user.resolve_username(bot_username).await.unwrap();
    let (emoji_id, _alt) = custom_emoji_with_alt(&bot).await;

    let (tx, mut rx) = mpsc::unbounded_channel();
    user.on_message(filters::private() & filters::user(bot_id), move |m| {
        let _ = tx.send(m);
        async { Ok(()) }
    })
    .group(1);

    let ids = Arc::new(Mutex::new(Vec::new()));
    let steps = tokio::spawn({
        let bot = bot.clone();
        let ids = ids.clone();
        let rec_path = rec_path.clone();
        async move {
            let inline_btn = InlineKeyboardButton::callback("Tap", "ce_btn")
                .style(ButtonStyle::Primary)
                .icon_custom_emoji_id(emoji_id);
            let inline_markup = InlineKeyboardMarkup::new([vec![inline_btn]]);
            let sent_inline =
                bot.send_message(user_id, "button icon check").reply_markup(inline_markup).await.unwrap_or_else(|e| {
                    panic!("bot sending inline button with icon_custom_emoji_id({emoji_id}) failed: {e:?}")
                });
            let user_inline = tokio::time::timeout(WAIT, rx.recv()).await.unwrap().unwrap();
            ids.lock().unwrap().push(user_inline.id);

            let reply_btn = KeyboardButton::new("Reply").icon_custom_emoji_id(emoji_id);
            let reply_markup = ReplyKeyboardMarkup::new([vec![reply_btn]]).one_time(true);
            let sent_reply =
                bot.send_message(user_id, "reply icon check").reply_markup(reply_markup).await.unwrap_or_else(|e| {
                    panic!("bot sending reply button with icon_custom_emoji_id({emoji_id}) failed: {e:?}")
                });
            let user_reply = tokio::time::timeout(WAIT, rx.recv()).await.unwrap().unwrap();
            ids.lock().unwrap().push(user_reply.id);

            // Prove the wire request carried each full button (style flags + icon id); `Message::from_updates` echoes
            // local `reply_markup` on `UpdateShortSentMessage`, so only the recording proves what went on the wire.
            let expected_inline =
                base::KeyboardInlineButton::KeyboardInlineButton(Box::new(types::KeyboardInlineButton {
                    style: Some(base::KeyboardButtonStyle::KeyboardButtonStyle(Box::new(types::KeyboardButtonStyle {
                        bg_primary: true,
                        bg_danger: false,
                        bg_success: false,
                        icon: Some(emoji_id),
                    }))),
                    text: "Tap".into(),
                    r#type: base::InlineButtonType::InlineButtonTypeCallback(Box::new(
                        types::InlineButtonTypeCallback { requires_password: false, data: b"ce_btn".to_vec() },
                    )),
                }))
                .to_bytes();
            let expected_reply = base::KeyboardButton::KeyboardButton(Box::new(types::KeyboardButton {
                style: Some(base::KeyboardButtonStyle::KeyboardButtonStyle(Box::new(types::KeyboardButtonStyle {
                    bg_primary: false,
                    bg_danger: false,
                    bg_success: false,
                    icon: Some(emoji_id),
                }))),
                text: "Reply".into(),
                r#type: base::ButtonType::ButtonTypeDefault,
            }))
            .to_bytes();
            let records = recording::read(&rec_path).unwrap();
            assert!(
                has_send_message_with(&records, &expected_inline),
                "outgoing messages.sendMessage did not carry serialized inline button with style and icon"
            );
            assert!(
                has_send_message_with(&records, &expected_reply),
                "outgoing messages.sendMessage did not carry serialized reply button with icon"
            );

            let Some(ReplyMarkup::Inline(read_inline)) = user_inline.reply_markup() else {
                panic!("expected inline reply markup on received message");
            };
            assert_eq!(read_inline.rows[0][0].text, "Tap");
            assert_eq!(read_inline.rows[0][0].style, ButtonStyle::Primary);

            let Some(ReplyMarkup::Keyboard(read_reply)) = user_reply.reply_markup() else {
                panic!("expected reply keyboard markup on received message");
            };
            assert_eq!(read_reply.rows[0][0].text, "Reply");

            // Re-fetch the bot's own messages via `get_messages` to inspect Telegram's stored copy rather than the
            // local echo.
            let bot_fetched = bot.get_messages(user_id, &[sent_inline.id, sent_reply.id]).await.unwrap();
            let bot_inline = bot_fetched[0].as_ref().expect("bot inline message missing from get_messages");
            let bot_reply = bot_fetched[1].as_ref().expect("bot reply message missing from get_messages");
            let Some(ReplyMarkup::Inline(stored_inline)) = bot_inline.reply_markup() else {
                panic!("expected inline reply markup on bot's stored message");
            };
            assert_eq!(stored_inline.rows[0][0].text, "Tap");
            assert_eq!(stored_inline.rows[0][0].style, ButtonStyle::Primary);

            let Some(ReplyMarkup::Keyboard(stored_reply)) = bot_reply.reply_markup() else {
                panic!("expected reply keyboard markup on bot's stored message");
            };
            assert_eq!(stored_reply.rows[0][0].text, "Reply");

            let bot_inline_icon = stored_inline.rows[0][0].icon_custom_emoji_id;
            let bot_reply_icon = stored_reply.rows[0][0].icon_custom_emoji_id;
            let user_inline_icon = read_inline.rows[0][0].icon_custom_emoji_id;
            let user_reply_icon = read_reply.rows[0][0].icon_custom_emoji_id;
            for (who, kind, got_icon) in [
                ("bot's stored copy", "inline", bot_inline_icon),
                ("bot's stored copy", "reply", bot_reply_icon),
                ("user's copy", "inline", user_inline_icon),
                ("user's copy", "reply", user_reply_icon),
            ] {
                match got_icon {
                    Some(id) => assert_eq!(id, emoji_id, "{who} {kind} button icon mismatch"),
                    None => eprintln!("Telegram dropped the icon ({who} / {kind})"),
                }
            }
        }
    })
    .await;

    let to_delete = ids.lock().unwrap().clone();
    if !to_delete.is_empty() {
        support::cleanup(user.delete_messages(bot_id, &to_delete).await, "deleting custom-emoji button messages");
    }
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
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
    let (emoji_id, alt) = custom_emoji_with_alt(&kit.bot).await;
    let placeholder = if alt == "👍" { "⭐" } else { "👍" };

    let (tx, mut rx) = mpsc::unbounded_channel();
    user.on_message(filters::private() & filters::user(bot_id), move |m| {
        let _ = tx.send(m);
        async { Ok(()) }
    })
    .group(1);

    let ids = Arc::new(Mutex::new(Vec::new()));
    let steps = tokio::spawn({
        let bot = kit.bot.clone();
        let user = user.clone();
        let ids = ids.clone();
        async move {
            // 1. Blocks with custom emoji + RichBlock::buttons
            let button = RichButton::new(&InlineKeyboardButton::callback("RichBtn", "rb")).unwrap();
            let blocks = [
                RichBlock::paragraph(Text::new().plain("emoji: ").custom_emoji(placeholder, emoji_id)),
                RichBlock::buttons([button]),
            ];
            let sent_blocks = bot.send_rich_message(user_id, InputRichMessage::blocks(blocks)).await.unwrap();
            let user_blocks = tokio::time::timeout(WAIT, rx.recv()).await.unwrap().unwrap();
            ids.lock().unwrap().push(user_blocks.id);
            assert!(sent_blocks.rich_message.is_some());

            let fetched = user.get_rich_message(bot_id, user_blocks.id).await.unwrap().expect("rich message exists");
            let rich = fetched.rich_message.expect("rich_message parsed");
            assert!(!rich.is_partial);
            let read_blocks = rich.blocks();
            assert_rich_emoji_paragraph(&read_blocks[0], "emoji: ", emoji_id, &alt, "blocks");
            assert!(
                matches!(&read_blocks[1].kind(), RichBlockKind::Buttons { buttons, .. } if buttons.len() == 1),
                "unexpected buttons block: {:?}",
                read_blocks[1].kind()
            );

            // Bots cannot call messages.getRichMessage.
            let bot_fetch_err = bot.get_rich_message(user_id, sent_blocks.id).await.unwrap_err();
            assert!(bot_fetch_err.is("BOT_METHOD_INVALID"), "expected BOT_METHOD_INVALID, got {bot_fetch_err:?}");

            // 2. HTML with <tg-emoji> and Markdown, then edit into rich blocks.
            let html_src = format!("<p>html <tg-emoji emoji-id=\"{emoji_id}\">{placeholder}</tg-emoji></p>");
            let _sent_html = bot.send_rich_message(user_id, InputRichMessage::html(html_src)).await.unwrap();
            let user_html = tokio::time::timeout(WAIT, rx.recv()).await.unwrap().unwrap();
            ids.lock().unwrap().push(user_html.id);
            let fetched_html = user.get_rich_message(bot_id, user_html.id).await.unwrap().unwrap();
            let html_blocks = fetched_html.rich_message.expect("html rich_message parsed").blocks();
            assert_rich_emoji_paragraph(&html_blocks[0], "html ", emoji_id, &alt, "html");

            let md_src = format!("**md** ![{placeholder}](tg://emoji?id={emoji_id})");
            let sent_md = bot.send_rich_message(user_id, InputRichMessage::markdown(md_src)).await.unwrap();
            let user_md = tokio::time::timeout(WAIT, rx.recv()).await.unwrap().unwrap();
            ids.lock().unwrap().push(user_md.id);
            let fetched_md = user.get_rich_message(bot_id, user_md.id).await.unwrap().unwrap();
            let md_blocks = fetched_md.rich_message.expect("md rich_message parsed").blocks();
            assert_rich_emoji_paragraph(&md_blocks[0], "md ", emoji_id, &alt, "markdown");

            let edited =
                sent_md.edit_text(InputRichMessage::blocks([RichBlock::paragraph("edited rich")])).await.unwrap();
            assert!(edited.rich_message.is_some());
            let fetched_edited =
                user.get_rich_message(bot_id, user_md.id).await.unwrap().expect("edited rich message exists");
            let edited_blocks = fetched_edited.rich_message.expect("edited rich_message parsed").blocks();
            assert_eq!(edited_blocks.len(), 1);
            assert!(
                matches!(&edited_blocks[0].kind(), RichBlockKind::Paragraph(t) if t.text() == "edited rich"),
                "unexpected edited rich blocks: {:?}",
                edited_blocks[0].kind()
            );
        }
    })
    .await;

    let to_delete = ids.lock().unwrap().clone();
    if !to_delete.is_empty() {
        support::cleanup(user.delete_messages(bot_id, &to_delete).await, "deleting rich message test messages");
    }
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
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

    let src_id = Arc::new(Mutex::new(None));
    let steps = tokio::spawn({
        let user = user.clone();
        let channel_id = channel.id;
        let src_id = src_id.clone();
        async move {
            let src = user.send_message("me", "copy to broadcast").await.unwrap();
            *src_id.lock().unwrap() = Some(src.id);
            let copied = user.copy_message(channel_id, "me", src.id).await.unwrap();
            assert_eq!(copied.chat.id, channel_id);
            assert_eq!(copied.text(), "copy to broadcast");
            assert!(copied.id > 0);

            let batch = user.copy_messages(channel_id, "me", &[src.id]).await.unwrap();
            assert_eq!(batch.len(), 1);
            assert!(batch[0] > 0);
        }
    })
    .await;

    let source = *src_id.lock().unwrap();
    if let Some(id) = source {
        support::cleanup(user.delete_messages("me", &[id]).await, "deleting copy source message");
    }
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
