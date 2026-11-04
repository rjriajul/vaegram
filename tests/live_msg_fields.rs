mod support;

use std::time::Duration;

use vaegram::types::Message;
use vaegram::{Client, filters};

const LINK: &str = "https://telegram.org";

async fn fetch(user: &Client, chat_id: i64, id: i32) -> Message {
    let found = user.get_messages(chat_id, &[id]).await.unwrap().pop().flatten();
    found.expect("the message is gone")
}

/// The bot echoes the preview url it received; a preview still loading shows up on a fetch a little later.
async fn link_previews(kit: &support::Kit) {
    kit.bot.on_message(filters::private() & filters::text(), |m| async move {
        let url = m.web_page.as_ref().map_or("pending".to_owned(), |p| p.url.clone());
        m.reply(url).await?;
        Ok(())
    });
    let reply = kit.user.send_and_wait_reply(&kit.bot, LINK).await.unwrap();
    if reply.text != "pending" {
        assert!(reply.text.starts_with(LINK), "the bot saw {:?}", reply.text);
        return;
    }
    let sent = reply.reply_to_message_id.expect("the bot's answer replies to the link message");
    for _ in 0..5 {
        tokio::time::sleep(Duration::from_secs(2)).await;
        if let Some(page) = fetch(&kit.user.client, reply.chat.id, sent).await.web_page {
            assert!(page.url.starts_with(LINK));
            return;
        }
    }
    panic!("the link preview never loaded");
}

async fn reactions(user: &Client, group: i64) {
    let sent = user.send_message(group, "react to me").await.unwrap();
    assert!(sent.reactions.is_none());
    sent.react("👍").await.unwrap();
    let reactions = fetch(user, group, sent.id).await.reactions.expect("no reactions after reacting");
    assert_eq!(reactions.reactions.iter().map(|r| r.total_count).sum::<i32>(), 1);
}

async fn scheduled_messages(user: &Client) {
    let date = i32::try_from(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs())
        .unwrap()
        + 3_600;
    let sent = user.send_message("me", "later").schedule_date(date).await.unwrap();
    assert!(sent.scheduled);
    let listed = user.get_scheduled_messages("me").await.unwrap();
    let cleanup = user.delete_scheduled_messages("me", &[sent.id]).await;
    assert!(listed.iter().any(|m| m.id == sent.id && m.scheduled));
    cleanup.unwrap();
}

#[tokio::test]
#[ignore = "creates and deletes a supergroup and schedules a message with a logged-in user session and a bot"]
async fn user_reads_previews_reactions_and_scheduled_messages() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let kit = std::sync::Arc::new(kit);
    let user = kit.user.client.clone();
    let group = user.create_supergroup("vaegram live test", "deleted right away").await.unwrap().id;
    // The steps run in their own task so a failed assertion still reaches the cleanup below.
    let steps = tokio::spawn({
        let kit = kit.clone();
        async move {
            link_previews(&kit).await;
            reactions(&kit.user.client, group).await;
            scheduled_messages(&kit.user.client).await;
        }
    })
    .await;
    user.delete_supergroup(group).await.unwrap();
    if let Err(failed) = steps {
        std::panic::resume_unwind(failed.into_panic());
    }
}
