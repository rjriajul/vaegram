#[allow(dead_code)]
mod support;

use std::future::IntoFuture;
use std::time::Duration;

use support::Kit;
use vaegram::filters;
use vaegram::types::{InlineKeyboardButton, InlineKeyboardMarkup};

const WAIT: Duration = Duration::from_secs(30);

#[tokio::test]
#[ignore = "uses a bot token and a logged-in user session"]
async fn wait_for_callback_query_gets_the_click() {
    let Some(kit) = Kit::from_env().await else { return };
    let bot = kit.bot.clone();
    kit.bot.on_message(filters::private() & filters::command("pick"), move |m| {
        let bot = bot.clone();
        async move {
            let keyboard = InlineKeyboardMarkup::new([vec![InlineKeyboardButton::callback("Pick", "pick")]]);
            let sent = m.reply("Pick one").reply_markup(keyboard).await?;
            let press = bot.wait_for_callback_query(m.chat.id).message_id(sent.id).timeout(WAIT).await?;
            press.answer().await?;
            m.reply(format!("picked {}", press.data)).await?;
            Ok(())
        }
    });

    let user = &kit.user.client;
    let asked = kit.user.send_and_wait_reply(&kit.bot, "/pick").await.unwrap();
    let picked = user.wait_for_message(asked.chat.id).filter(filters::text()).timeout(WAIT).into_future();
    let click = user.request_callback_answer(asked.chat.id, asked.id, "pick").into_future();
    let (picked, click) = tokio::join!(picked, click);
    let picked = picked.unwrap();
    user.delete_messages(asked.chat.id, &[asked.id, picked.id]).await.unwrap();
    assert!(click.is_ok(), "{click:?}");
    assert_eq!(picked.text, "picked pick");
}

#[tokio::test]
#[ignore = "uses a bot token and a logged-in user session"]
async fn ask_returns_the_users_answer() {
    let Some(kit) = Kit::from_env().await else { return };
    let bot = kit.bot.clone();
    kit.bot.on_message(filters::private() & filters::command("ask"), move |m| {
        let bot = bot.clone();
        async move {
            let answer = bot.ask(m.chat.id, "Your name?").filter(filters::text()).timeout(WAIT).await?;
            answer.reply(format!("Hi {}", answer.text)).await?;
            Ok(())
        }
    });

    let question = kit.user.send_and_wait_reply(&kit.bot, "/ask").await.unwrap();
    let greeting = kit.user.send_and_wait_reply(&kit.bot, "Ada").await.unwrap();
    kit.user.client.delete_messages(question.chat.id, &[question.id, greeting.id]).await.unwrap();
    assert_eq!(question.text, "Your name?");
    assert_eq!(greeting.text, "Hi Ada");
}
