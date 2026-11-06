#[allow(dead_code)]
mod support;

use std::time::Duration;

use tokio::sync::mpsc;
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

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn a_tz_offset_client_connects() {
    let Some(builder) = user_builder() else { return };
    let client = builder.tz_offset(3600).lang_pack("tdesktop").connect().await.unwrap();
    assert!(client.get_me().await.unwrap().is_self);
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot token"]
async fn compose_runs_a_bot_and_a_user() {
    let (Some(builder), Ok(token)) = (user_builder(), std::env::var("VAEGRAM_BOT_TOKEN")) else {
        eprintln!("skipped: set VAEGRAM_BOT_TOKEN too");
        return;
    };
    let user = builder.connect().await.unwrap();
    let (api_id, api_hash) = (env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    let bot = Client::builder("target/vaegram-compose-bot", api_id, api_hash).bot_token(token).start().await.unwrap();
    bot.on_message(filters::private(), |m| async move {
        m.reply("echo").await?;
        Ok(())
    });
    let (tx, mut replies) = mpsc::unbounded_channel();
    user.on_message(filters::private(), move |m| {
        if !m.outgoing {
            let _ = tx.send(m.chat.id);
        }
        async { Ok(()) }
    });
    let composed = tokio::spawn({
        let clients = [user.clone(), bot.clone()];
        async move { Client::compose(&clients).await }
    });

    let me = bot.get_me().await.unwrap();
    let bot_id = user.resolve_username(&me.username.unwrap()).await.unwrap();
    user.send_message(bot_id, "ping").await.unwrap();
    assert_eq!(tokio::time::timeout(WAIT, replies.recv()).await.unwrap(), Some(bot_id));
    user.stop();
    bot.stop();
    assert!(tokio::time::timeout(WAIT, composed).await.unwrap().unwrap().is_ok());
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn counts_match_the_lists() {
    let Some(builder) = user_builder() else { return };
    let client = builder.connect().await.unwrap();
    let photos = client.get_chat_photos("me").await.unwrap().len() as i32;
    assert_eq!(client.get_chat_photos_count("me").await.unwrap(), photos);
    let audios = client.get_user_profile_audios("me").await.unwrap().len() as i32;
    assert_eq!(client.get_chat_audios_count("me").await.unwrap(), audios);
}
