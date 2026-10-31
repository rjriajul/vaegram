mod support;

use vaegram::Client;
use vaegram::filters;
use vaegram::types::{InputChecklist, InputRichMessage, Message, Upload};

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

async fn pins_cached_media_and_post_counts(user: &Client, group: i64) {
    let pinned = |m: Option<Message>| m.map(|m| m.id);
    let sent = user.send_message(group, "pin me").await.unwrap();
    user.pin_chat_message(group, sent.id).await.unwrap();
    assert_eq!(pinned(user.get_pinned_message(group).await.unwrap()), Some(sent.id));
    user.unpin_chat_message(group, sent.id).await.unwrap();
    // Nothing else in the fresh group was ever pinned.
    assert_eq!(pinned(user.get_pinned_message(group).await.unwrap()), None);

    let document = user.send_document(group, Upload::bytes("a.txt", b"hi")).await.unwrap();
    let file_id = document.document().expect("not a document").file_id.clone();
    let again = user.send_cached_media(group, file_id).caption("again").await.unwrap();
    assert!(again.document().is_some());

    assert!(user.search_posts_count("#telegram").await.unwrap() >= 0);
}

#[tokio::test]
#[ignore = "creates and deletes a supergroup with a logged-in user session and a bot"]
async fn user_uses_message_extras() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let kit = std::sync::Arc::new(kit);
    let user = kit.user.client.clone();
    let group = user.create_supergroup("vaegram live test", "deleted right away").await.unwrap().id;
    // The steps run in their own task so a failed assertion still reaches the cleanup below.
    let steps = tokio::spawn({
        let kit = kit.clone();
        async move {
            let user = &kit.user.client;
            polls(user, group).await;
            screenshots_and_rich_messages(&kit, group).await;
            pins_cached_media_and_post_counts(user, group).await;
            if premium() {
                checklists(user, group).await;
                text_tools(user).await;
            }
        }
    })
    .await;
    user.delete_supergroup(group).await.unwrap();
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}
