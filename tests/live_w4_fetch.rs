use std::future::IntoFuture;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::mpsc;
use vaegram::raw::base;
use vaegram::types::Media;
use vaegram::{Client, filters};

const WAIT: Duration = Duration::from_secs(30);

fn credentials() -> Option<(String, String, i32, String)> {
    let (Ok(session), Ok(token)) = (std::env::var("VAEGRAM_USER_SESSION"), std::env::var("VAEGRAM_BOT_TOKEN")) else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION and VAEGRAM_BOT_TOKEN");
        return None;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    Some((session, token, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH")))
}

async fn bot_id(bot: &Client, user: &Client) -> i64 {
    let base::User::User(me) = bot.get_me().await.unwrap() else { panic!("the bot has no user") };
    user.resolve_username(&me.username.unwrap()).await.unwrap()
}

#[tokio::test]
#[ignore = "uses a bot token and a logged-in user session"]
async fn fetch_replies_needs_no_request_in_the_handler() {
    let Some((session, token, api_id, api_hash)) = credentials() else { return };
    let fetches = Arc::new(AtomicUsize::new(0));
    let counted = fetches.clone();
    let bot = Client::builder("target/vaegram-fetch-bot", api_id, api_hash.clone())
        .bot_token(token)
        .fetch_replies(true)
        .observer(move |call| {
            if call.name == "messages.getMessages" {
                counted.fetch_add(1, Ordering::Relaxed);
            }
        })
        .start()
        .await
        .unwrap();
    let user = Client::builder(session, api_id, api_hash).connect().await.unwrap();

    let (tx, mut seen) = mpsc::unbounded_channel();
    let before = fetches.clone();
    bot.on_message(filters::private() & filters::text(), move |m| {
        let (tx, before) = (tx.clone(), before.clone());
        async move {
            if m.reply_to_message_id.is_some() {
                let asked = before.load(Ordering::Relaxed);
                let replied = m.reply_to_message().await?.map(|r| r.text().to_owned());
                let _ = tx.send((replied, before.load(Ordering::Relaxed) - asked));
            }
            Ok(())
        }
    });
    let runner = bot.clone();
    tokio::spawn(async move { runner.run().await });

    let bot_id = bot_id(&bot, &user).await;
    let first = user.send_message(bot_id, "first").into_future().await.unwrap();
    let second = user.send_message(bot_id, "second").reply_to(first.id).into_future().await.unwrap();
    let got = tokio::time::timeout(WAIT, seen.recv()).await;
    user.delete_messages(bot_id, &[first.id, second.id]).await.unwrap();
    // The fetch happened before the handler ran, so the handler's own read sent nothing.
    assert_eq!(got.unwrap(), Some((Some("first".to_owned()), 0)));
}

#[tokio::test]
#[ignore = "uses a bot token and a logged-in user session"]
async fn fetch_stickers_names_the_set() {
    let Some((session, token, api_id, api_hash)) = credentials() else { return };
    let set_name = std::env::var("VAEGRAM_STICKER_SET").unwrap_or_else(|_| "Animals".to_owned());
    let bot = Client::builder("target/vaegram-fetch-bot", api_id, api_hash.clone())
        .bot_token(token)
        .fetch_stickers(true)
        .start()
        .await
        .unwrap();
    let user = Client::builder(session, api_id, api_hash).connect().await.unwrap();

    let (tx, mut seen) = mpsc::unbounded_channel();
    bot.on_message(filters::private() & filters::sticker(), move |m| {
        let tx = tx.clone();
        async move {
            let name = match &m.media {
                Some(Media::Sticker(sticker)) => sticker.set_name.clone(),
                _ => None,
            };
            let _ = tx.send(name);
            Ok(())
        }
    });
    let runner = bot.clone();
    tokio::spawn(async move { runner.run().await });

    let bot_id = bot_id(&bot, &user).await;
    let set = user.get_sticker_set(&set_name).await.unwrap();
    let sent = user.send_sticker(bot_id, &set.stickers[0]).into_future().await.unwrap();
    let got = tokio::time::timeout(WAIT, seen.recv()).await;
    user.delete_messages(bot_id, &[sent.id]).await.unwrap();
    assert_eq!(got.unwrap(), Some(Some(set.name)));
}
