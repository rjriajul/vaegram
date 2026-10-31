mod support;

use vaegram::Client;
use vaegram::filters;
use vaegram::types::{InputChecklist, InputRichMessage, Message};

fn premium() -> bool {
    let premium = std::env::var("VAEGRAM_USER_PREMIUM").is_ok();
    if !premium {
        eprintln!("skipped the Premium steps: set VAEGRAM_USER_PREMIUM for a Premium user session");
    }
    premium
}

async fn fetch(user: &Client, chat_id: i64, id: i32) -> Message {
    let found = user.get_messages(chat_id, &[id]).await.unwrap().pop().flatten();
    found.expect("the message is gone")
}

async fn checklists(user: &Client, group: i64) {
    let checklist =
        InputChecklist::new("Trip").task(1, "Tickets").others_can_add_tasks().others_can_mark_tasks_as_done();
    let sent = user.send_checklist(group, checklist).await.unwrap();
    user.add_checklist_tasks(group, sent.id).task(2, "Visa").await.unwrap();
    user.mark_checklist_tasks_as_done(group, sent.id, &[1], &[]).await.unwrap();
    let message = fetch(user, group, sent.id).await;
    let tasks = &message.checklist().expect("not a checklist").tasks;
    assert_eq!(tasks.len(), 2);
    assert!(tasks.iter().any(|t| t.id == 1 && t.completion_date.is_some()));
}

async fn polls(user: &Client, group: i64) {
    let sent = user.send_poll(group, "Lunch?", ["A", "B"]).await.unwrap();
    let poll = user.get_poll_results(group, sent.id).await.unwrap();
    assert_eq!((poll.question.as_str(), poll.options.len()), ("Lunch?", 2));
}

async fn screenshots_and_rich_messages(kit: &support::Kit, group: i64) {
    kit.bot.on_message(filters::private() & filters::text(), |m| async move {
        m.reply("ok").await?;
        Ok(())
    });
    let reply = kit.user.send_and_wait_reply(&kit.bot, "hi").await.unwrap();
    let user = &kit.user.client;
    user.send_screenshot_notification(reply.chat.id).await.unwrap();

    let sent = user.send_rich_message(group, InputRichMessage::html("<b>hi</b>")).await.unwrap();
    let found = user.get_rich_message(group, sent.id).await.unwrap().expect("the rich message is gone");
    assert!(found.rich_message.is_some_and(|r| !r.part));
}

async fn text_tools(user: &Client) {
    let fixed = user.fix_text_with_ai("Ths is a tset").await.unwrap();
    assert!(!fixed.text.text.is_empty());
    user.compose_text_with_ai("hello").to_language_code("es").await.unwrap();
}

#[tokio::test]
#[ignore = "creates and deletes a supergroup with a logged-in user session and a bot"]
async fn user_uses_message_extras() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = &kit.user.client;
    let group = user.create_supergroup("vaegram live test", "deleted right away").await.unwrap().id;
    polls(user, group).await;
    screenshots_and_rich_messages(&kit, group).await;
    if premium() {
        checklists(user, group).await;
        text_tools(user).await;
    }
    user.delete_supergroup(group).await.unwrap();
}
