mod support;

use std::future::IntoFuture;
use std::time::Duration;

use vaegram::types::{InlineKeyboardButton, InlineKeyboardMarkup, InputMedia, Upload};
use vaegram::{Client, filters};

const WAIT: Duration = Duration::from_secs(30);

async fn polls(user: &Client, group: i64) {
    let sent = user.send_poll(group, "Lunch?", ["A", "B"]).await.unwrap();
    sent.vote(&[0]).await.unwrap();
    sent.retract_vote().await.unwrap();
    sent.view().await.unwrap();
}

async fn albums_and_links(user: &Client, group: i64) {
    let album =
        [InputMedia::document(Upload::bytes("a.txt", b"a")), InputMedia::document(Upload::bytes("b.txt", b"b"))];
    let sent = user.send_media_group(group, album).await.unwrap();
    let found = sent[0].get_media_group().await.unwrap();
    assert_eq!(found.iter().map(|m| m.id).collect::<Vec<_>>(), sent.iter().map(|m| m.id).collect::<Vec<_>>());

    let exported = user.export_message_link(group, sent[0].id).await.unwrap();
    assert_eq!(sent[0].link().as_deref(), Some(exported.as_str()));
}

#[tokio::test]
#[ignore = "creates and deletes a supergroup with a logged-in user session and a bot"]
async fn user_votes_views_and_links_messages() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = kit.user.client.clone();
    let Some(group) = support::test_supergroup(&user).await else { return };
    // The steps run in their own task so a failed assertion still reaches the cleanup below.
    let steps = tokio::spawn({
        let user = user.clone();
        async move {
            polls(&user, group).await;
            albums_and_links(&user, group).await;
        }
    })
    .await;
    support::cleanup(user.delete_supergroup(group).await, "deleting the supergroup \"vaegram live test\"");
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}

#[tokio::test]
#[ignore = "uses a bot token and a logged-in user session"]
async fn bot_edits_the_media_it_sent() {
    let Some(kit) = support::Kit::from_env().await else { return };
    kit.bot.on_message(filters::private() & filters::command("edit"), |m| async move {
        let sent = m.client().send_document(m.chat.id, Upload::bytes("old.txt", b"old")).await?;
        sent.edit_media(InputMedia::document(Upload::bytes("new.txt", b"new"))).await?;
        m.reply("edited").await?;
        Ok(())
    });

    let user = &kit.user.client;
    let me = kit.bot.get_me().await.unwrap();
    let bot_id = user.resolve_username(&me.username.expect("the bot has no username")).await.unwrap();
    // The bot's document arrives before its text answer, so the user waits for the text.
    let done = user.wait_for_message(bot_id).filter(filters::text()).timeout(WAIT).into_future();
    let (done, asked) = tokio::join!(done, user.send_message(bot_id, "/edit").into_future());
    let (done, _) = (done.unwrap(), asked.unwrap());
    let mut pages = user.get_chat_history(done.chat.id).limit(3);
    let mut history = Vec::new();
    while let Some(message) = pages.next().await.unwrap() {
        history.push(message);
    }
    let file_names: Vec<_> = history.iter().filter_map(|m| m.document().and_then(|d| d.file_name.clone())).collect();
    let ids: Vec<_> = history.iter().map(|m| m.id).collect();
    user.delete_messages(done.chat.id, &ids).await.unwrap();
    assert_eq!(done.text(), "edited");
    assert_eq!(file_names, ["new.txt"]);
}

#[tokio::test]
#[ignore = "uses a bot token and a logged-in user session"]
async fn wait_for_click_gets_the_press_on_the_sent_keyboard() {
    let Some(kit) = support::Kit::from_env().await else { return };
    kit.bot.on_message(filters::private() & filters::command("click"), |m| async move {
        let keyboard = InlineKeyboardMarkup::new([vec![InlineKeyboardButton::callback("Pick", "pick")]]);
        let sent = m.reply("Pick one").reply_markup(keyboard).await?;
        let press = sent.wait_for_click().timeout(WAIT).await?;
        press.answer().await?;
        m.reply(format!("clicked {}", press.data)).await?;
        Ok(())
    });

    let user = &kit.user.client;
    let asked = kit.user.send_and_wait_reply(&kit.bot, "/click").await.unwrap();
    let clicked = user.wait_for_message(asked.chat.id).filter(filters::text()).timeout(WAIT).into_future();
    let click = user.request_callback_answer(asked.chat.id, asked.id, "pick").into_future();
    let (clicked, click) = tokio::join!(clicked, click);
    let clicked = clicked.unwrap();
    user.delete_messages(asked.chat.id, &[asked.id, clicked.id]).await.unwrap();
    assert!(click.is_ok(), "{click:?}");
    assert_eq!(clicked.text(), "clicked pick");
}
