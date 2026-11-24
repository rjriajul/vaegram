mod support;

use std::future::IntoFuture;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Notify;

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
        let sent = m.reply_document(Upload::bytes("old.txt", b"old")).await?;
        sent.edit_media(InputMedia::document(Upload::bytes("new.txt", b"new"))).await?;
        m.reply("edited").await?;
        Ok(())
    });

    // Both answers reply to the command, and the user's handlers may pass them on in either order; the document
    // was sent first, so it has the lower id.
    let first = kit.user.send_and_wait_reply(&kit.bot, "/edit").await.unwrap();
    let asked = first.reply_to_message_id.expect("the answer replies to the command");
    let second = kit.user.wait_reply(first.chat.id, asked).await.unwrap();
    let mut answers = [first, second];
    answers.sort_by_key(|m| m.id);
    let [document, done] = answers;
    let user = &kit.user.client;
    let edited = user.get_messages(document.chat.id, &[document.id]).await.unwrap().pop().flatten();
    let file_name = edited.and_then(|m| m.document().and_then(|d| d.file_name.clone()));
    user.delete_messages(document.chat.id, &[asked, document.id, done.id]).await.unwrap();
    assert_eq!(done.text(), "edited");
    assert_eq!(file_name.as_deref(), Some("new.txt"));
}

#[tokio::test]
#[ignore = "uses a bot token and a logged-in user session"]
async fn wait_for_click_gets_the_press_on_the_sent_keyboard() {
    let Some(kit) = support::Kit::from_env().await else { return };
    // Telegram may show the keyboard to the user before it answers the bot's send, so a press can arrive before the
    // bot listens for it and go unclaimed. The user presses only once the bot's listener is in place: its first poll
    // registers it, since a known chat id resolves without a request.
    let listening = Arc::new(Notify::new());
    let ready = listening.clone();
    kit.bot.on_message(filters::private() & filters::command("click"), move |m| {
        let ready = ready.clone();
        async move {
            let keyboard = InlineKeyboardMarkup::new([vec![InlineKeyboardButton::callback("Pick", "pick")]]);
            let sent = m.reply("Pick one").reply_markup(keyboard).await?;
            let mut press = std::pin::pin!(sent.wait_for_click().timeout(WAIT).into_future());
            tokio::select! {
                biased;
                early = &mut press => early.map(drop)?,
                () = std::future::ready(()) => {}
            }
            ready.notify_one();
            let press = press.await?;
            press.answer().await?;
            m.reply(format!("clicked {}", press.data)).await?;
            Ok(())
        }
    });

    let user = &kit.user.client;
    let asked = kit.user.send_and_wait_reply(&kit.bot, "/click").await.unwrap();
    tokio::time::timeout(WAIT, listening.notified()).await.expect("the bot never listened for the press");
    let command = asked.reply_to_message_id.expect("the keyboard replies to the command");
    let clicked = kit.user.wait_reply(asked.chat.id, command);
    let click = user.request_callback_answer(asked.chat.id, asked.id, "pick").into_future();
    let (clicked, click) = tokio::join!(clicked, click);
    let clicked = clicked.unwrap();
    user.delete_messages(asked.chat.id, &[asked.id, clicked.id]).await.unwrap();
    assert!(click.is_ok(), "{click:?}");
    assert_eq!(clicked.text(), "clicked pick");
}
