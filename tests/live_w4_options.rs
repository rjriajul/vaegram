#[allow(dead_code)]
mod support;

use std::time::Duration;

use vaegram::enums::{MessageEntityType, ParseMode};
use vaegram::types::{Json, Upload};
use vaegram::{Client, ClientBuilder, filters};

const WAIT: Duration = Duration::from_secs(30);

fn env(key: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| panic!("set {key}"))
}

fn user_builder() -> Option<ClientBuilder> {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return None;
    };
    Some(Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH")))
}

/// A running bot that answers every private message with "echo", and the id a user finds it by.
async fn echo_bot(user: &Client) -> Option<(Client, i64)> {
    let Ok(token) = std::env::var("VAEGRAM_BOT_TOKEN") else {
        eprintln!("skipped: set VAEGRAM_BOT_TOKEN too");
        return None;
    };
    let (api_id, api_hash) = (env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    let bot =
        Client::builder("target/vaegram-w4-options-bot", api_id, api_hash).bot_token(token).start().await.unwrap();
    bot.on_message(filters::private(), |m| async move {
        m.reply("echo").await?;
        Ok(())
    });
    let me = bot.get_me().await.unwrap();
    let bot_id = user.resolve_username(&me.username.unwrap()).await.unwrap();
    Some((bot, bot_id))
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn ask_accepts_an_answer_in_a_second_chat() {
    let Some(builder) = user_builder() else { return };
    let user = builder.connect().await.unwrap();
    let Some((bot, bot_id)) = echo_bot(&user).await else { return };
    user.on_message(filters::private(), |_| async { Ok(()) });
    let running = tokio::spawn({
        let clients = [user.clone(), bot.clone()];
        async move { Client::compose(&clients).await }
    });

    // The question goes to Saved Messages, where nobody answers; the bot's answer in its own chat counts.
    let asked = tokio::spawn({
        let user = user.clone();
        async move { user.ask("me", "answer in the bot's chat").chat(bot_id).timeout(WAIT).await }
    });
    tokio::time::sleep(Duration::from_secs(2)).await;
    user.send_message(bot_id, "ping").await.unwrap();
    let answer = tokio::time::timeout(WAIT, asked).await.unwrap().unwrap().unwrap();
    assert_eq!(answer.chat.id, bot_id);
    assert!(!answer.outgoing, "our own question isn't the answer");

    user.stop();
    bot.stop();
    assert!(tokio::time::timeout(WAIT, running).await.unwrap().unwrap().is_ok());
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn set_parse_mode_switches_markup() {
    let Some(builder) = user_builder() else { return };
    let client = builder.connect().await.unwrap();
    client.set_parse_mode(ParseMode::Html);
    let marked = client.send_message("me", "<b>bold</b>").await.unwrap();
    client.set_parse_mode(ParseMode::Disabled);
    let literal = client.send_message("me", "<b>bold</b>").await.unwrap();
    client.delete_messages("me", &[marked.id, literal.id]).await.unwrap();

    assert_eq!(marked.text(), "bold");
    let entities = marked.entities().unwrap_or_default();
    assert!(entities.iter().any(|e| matches!(e.kind, MessageEntityType::Bold)), "{entities:?}");
    assert_eq!(literal.text(), "<b>bold</b>");
    assert!(literal.entities().is_none());
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn two_downloads_with_one_slot_both_finish() {
    let Some(builder) = user_builder() else { return };
    let client = builder.max_concurrent_transmissions(1).connect().await.unwrap();
    let (first, second) = (vec![1u8; 700 * 1024], vec![2u8; 700 * 1024]);
    let one = client.send_document("me", Upload::bytes("one.bin", &first)).await.unwrap();
    let two = client.send_document("me", Upload::bytes("two.bin", &second)).await.unwrap();
    let (a, b) = (one.document().unwrap(), two.document().unwrap());

    let got =
        tokio::time::timeout(WAIT * 2, async { tokio::join!(client.download_media(a), client.download_media(b)) })
            .await;
    client.delete_messages("me", &[one.id, two.id]).await.unwrap();
    let (got_first, got_second) = got.unwrap();
    assert!(got_first.unwrap() == first && got_second.unwrap() == second);
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn compose_sequential_runs_bot_and_user() {
    let Some(builder) = user_builder() else { return };
    let user = builder.connect().await.unwrap();
    let Some((bot, bot_id)) = echo_bot(&user).await else { return };
    let (tx, mut replies) = tokio::sync::mpsc::unbounded_channel();
    user.on_message(filters::private(), move |m| {
        if !m.outgoing {
            let _ = tx.send(m.chat.id);
        }
        async { Ok(()) }
    });
    let composed = tokio::spawn({
        let clients = [user.clone(), bot.clone()];
        async move { Client::compose(&clients).sequential().await }
    });

    user.send_message(bot_id, "ping").await.unwrap();
    assert_eq!(tokio::time::timeout(WAIT, replies.recv()).await.unwrap(), Some(bot_id));
    user.stop();
    bot.stop();
    assert!(tokio::time::timeout(WAIT, composed).await.unwrap().unwrap().is_ok());
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn custom_init_connection_params_connect() {
    let Some(builder) = user_builder() else { return };
    let params = Json::Object(vec![("tz_offset".into(), Json::Number(3600.0)), ("x".into(), Json::String("y".into()))]);
    let client = builder.init_connection_params(params).connect().await.unwrap();
    assert!(client.get_me().await.unwrap().is_self);
}
