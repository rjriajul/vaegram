//! The Bot API transport against the real api.telegram.org. Ignored by default; run with
//! `cargo test --features bot-api --test live_bot_api -- --ignored`. Each test needs `VAEGRAM_BOT_TOKEN` and
//! `VAEGRAM_CHAT_ID` (a chat the bot may post in) and skips itself without them. The token is never printed.
#![cfg(feature = "bot-api")]

use std::io::Write;
use std::time::Duration;

use vaegram::bot_api::BotApi;
use vaegram::bot_api::raw::methods::{GetMe, GetUpdates};
use vaegram::types::Upload;
use vaegram::{Client, filters};

/// The bot's token and the chat to post in, or `None` with a note when either is unset.
fn env() -> Option<(String, i64)> {
    let (Ok(token), Ok(chat)) = (std::env::var("VAEGRAM_BOT_TOKEN"), std::env::var("VAEGRAM_CHAT_ID")) else {
        eprintln!("skipped: set VAEGRAM_BOT_TOKEN and VAEGRAM_CHAT_ID");
        return None;
    };
    Some((token, chat.parse().expect("VAEGRAM_CHAT_ID is a Bot API chat id")))
}

async fn client(token: &str) -> Client {
    // The Bot API needs no api_id or api_hash.
    Client::builder("vaegram-live-bot-api", 1, "unused").bot_api(token).start().await.expect("getMe failed")
}

/// A `side` by `side` PNG, a gradient, made here so the test needs no file.
fn png(side: u32) -> Vec<u8> {
    let mut pixels = Vec::new();
    for y in 0..side {
        // Each row starts with its filter type: none.
        pixels.push(0);
        for x in 0..side {
            pixels.extend_from_slice(&[(x * 255 / side) as u8, (y * 255 / side) as u8, 160]);
        }
    }
    let mut zlib = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    zlib.write_all(&pixels).unwrap();
    let image = zlib.finish().unwrap();
    let mut header = Vec::new();
    header.extend_from_slice(&side.to_be_bytes());
    header.extend_from_slice(&side.to_be_bytes());
    // 8 bits per channel, RGB, deflate, no filter method extras, no interlace.
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    for (kind, data) in [(b"IHDR", header), (b"IDAT", image), (b"IEND", Vec::new())] {
        png.extend_from_slice(&(data.len() as u32).to_be_bytes());
        let mut crc = crc32fast::Hasher::new();
        crc.update(kind);
        crc.update(&data);
        png.extend_from_slice(kind);
        png.extend_from_slice(&data);
        png.extend_from_slice(&crc.finalize().to_be_bytes());
    }
    png
}

#[tokio::test]
#[ignore = "calls the real Bot API"]
async fn get_me_answers_the_same_bot_raw_and_friendly() {
    let Some((token, _)) = env() else { return };
    let raw = BotApi::new(token.as_str()).unwrap().invoke(&GetMe {}).await.unwrap();
    let me = client(&token).await.get_me().await.unwrap();
    assert!(me.is_bot && raw.is_bot);
    assert_eq!(me.id, raw.id);
    assert_eq!(me.username.as_deref(), raw.username.as_deref());
}

#[tokio::test]
#[ignore = "calls the real Bot API and posts in VAEGRAM_CHAT_ID"]
async fn sends_edits_and_deletes_a_message() {
    let Some((token, chat_id)) = env() else { return };
    let app = client(&token).await;
    let sent = app.send_message(chat_id, "hello over the Bot API").await.unwrap();
    assert_eq!((sent.chat.id, sent.text(), sent.outgoing), (chat_id, "hello over the Bot API", true));

    let edited = app.edit_message_text(chat_id, sent.id, "edited over the Bot API").await.unwrap();
    assert_eq!((edited.id, edited.text()), (sent.id, "edited over the Bot API"));
    assert!(edited.edit_date.is_some());

    app.delete_messages(chat_id, &[sent.id]).await.unwrap();
    // Gone: editing it again fails.
    assert!(app.edit_message_text(chat_id, sent.id, "too late").await.is_err());
}

#[tokio::test]
#[ignore = "calls the real Bot API and posts in VAEGRAM_CHAT_ID"]
async fn sends_a_photo_from_bytes() {
    let Some((token, chat_id)) = env() else { return };
    let app = client(&token).await;
    let image = png(128);
    let sent = app.send_photo(chat_id, Upload::bytes("vaegram.png", &image)).caption("a photo from bytes").await;
    let sent = sent.unwrap();
    assert_eq!(sent.caption(), Some("a photo from bytes"));
    let photo = sent.photo().expect("a photo");
    assert!(photo.width > 0 && photo.height > 0, "{photo:?}");
    app.delete_messages(chat_id, &[sent.id]).await.unwrap();
}

#[tokio::test]
#[ignore = "calls the real Bot API"]
async fn one_poll_round_then_a_clean_stop() {
    let Some((token, _)) = env() else { return };
    let app = client(&token).await;
    // A bot with a webhook can't poll; that bot's webhook isn't this test's to remove.
    let info = app.get_webhook_info().await.unwrap();
    if !info.url.is_empty() {
        eprintln!("skipped: the bot has a webhook set, so getUpdates would fail");
        return;
    }
    // A raw round without an offset confirms nothing, so no update is lost to the bot's other users.
    let raw = BotApi::new(token.as_str()).unwrap();
    let updates = raw.invoke(&GetUpdates { timeout: Some(0), limit: Some(1), ..GetUpdates::default() }).await;
    assert!(updates.is_ok(), "{:?}", updates.err());

    app.on_message(filters::all(), |_| async { Ok::<_, vaegram::Error>(()) });
    let runner = app.clone();
    let run = tokio::spawn(async move { runner.run().await });
    // Long enough for the first poll to go out and be held open by Telegram.
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert!(!run.is_finished(), "run ended before stop");
    app.stop();
    tokio::time::timeout(Duration::from_secs(20), run).await.expect("run didn't stop").unwrap().unwrap();
}
