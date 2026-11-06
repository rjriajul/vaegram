use std::future::IntoFuture;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::mpsc;
use vaegram::types::{InlineKeyboardButton, InlineKeyboardMarkup};
use vaegram::{Client, filters};

const WAIT: Duration = Duration::from_secs(30);

#[tokio::test]
#[ignore = "uses a bot token and a logged-in user session"]
async fn callback_on_a_cached_message_needs_no_request() {
    let (Ok(session), Ok(token)) = (std::env::var("VAEGRAM_USER_SESSION"), std::env::var("VAEGRAM_BOT_TOKEN")) else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION and VAEGRAM_BOT_TOKEN");
        return;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let (api_id, api_hash): (i32, String) = (env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    let fetches = Arc::new(AtomicUsize::new(0));
    let counted = fetches.clone();
    let bot = Client::builder("target/vaegram-caches-bot", api_id, api_hash.clone())
        .bot_token(token)
        .max_message_cache_size(100)
        .observer(move |call| {
            if call.name == "messages.getMessages" {
                counted.fetch_add(1, Ordering::Relaxed);
            }
        })
        .start()
        .await
        .unwrap();
    let user = Client::builder(session, api_id, api_hash).connect().await.unwrap();

    bot.on_message(filters::private() & filters::command("tap"), |m| async move {
        let keyboard = InlineKeyboardMarkup::new([vec![InlineKeyboardButton::callback("Tap", "tap")]]);
        m.reply("Tap it").reply_markup(keyboard).await?;
        Ok(())
    });
    let (tx, mut pressed) = mpsc::unbounded_channel();
    bot.on_callback_query(filters::all(), move |q| {
        let tx = tx.clone();
        async move {
            let _ = tx.send(q.message().await?.map(|m| m.text().to_owned()));
            q.answer().await?;
            Ok(())
        }
    });
    for client in [&user, &bot] {
        let client = client.clone();
        tokio::spawn(async move { client.run().await });
    }

    let me = bot.get_me().await.unwrap();
    let bot_id = user.resolve_username(&me.username.unwrap()).await.unwrap();
    let keyboard = user.wait_for_message(bot_id).filter(filters::text()).timeout(WAIT).into_future();
    let (keyboard, sent) = tokio::join!(keyboard, user.send_message(bot_id, "/tap").into_future());
    let (keyboard, sent) = (keyboard.unwrap(), sent.unwrap());
    let _ = user.request_callback_answer(bot_id, keyboard.id, "tap").await;
    let seen = tokio::time::timeout(WAIT, pressed.recv()).await;
    user.delete_messages(bot_id, &[sent.id, keyboard.id]).await.unwrap();
    // The bot sent the keyboard itself, so the cache already had the message the button is on.
    assert_eq!(seen.unwrap(), Some(Some("Tap it".to_owned())));
    assert_eq!(fetches.load(Ordering::Relaxed), 0);
}
