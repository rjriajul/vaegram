#[allow(dead_code)]
mod support;

use vaegram::enums::StoriesPrivacyRules;
use vaegram::types::{InputStoryContent, Upload};
use vaegram::{Client, filters};

#[tokio::test]
#[ignore = "uses a logged-in user session and a bot"]
async fn bot_replies_and_answers_with_media() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    kit.bot.on_message(filters::private() & filters::text(), move |m| {
        let tx = tx.clone();
        async move {
            if m.text() == "media" {
                let document = m.reply_document(Upload::bytes("a.txt", b"a")).await.map(drop);
                let poll = m.answer_poll("q", ["a", "b"]).await.map(drop);
                let dice = m.reply_dice().await.map(drop);
                let _ = tx.send([document, poll, dice]);
            }
            Ok(())
        }
    });

    let reply = kit.user.send_and_wait_reply(&kit.bot, "media").await.unwrap();
    assert!(reply.document().is_some(), "the first reply is the document");
    let replied = reply.reply_to_message().await.unwrap().expect("the document replies to the request");
    assert_eq!(replied.text(), "media");
    let sent = tokio::time::timeout(std::time::Duration::from_secs(30), rx.recv()).await.unwrap().unwrap();
    for result in sent {
        result.unwrap();
    }
}

async fn user_client() -> Option<Client> {
    let Ok(session) = std::env::var("VAEGRAM_USER_SESSION") else {
        eprintln!("skipped: set VAEGRAM_USER_SESSION (a logged-in session name)");
        return None;
    };
    let env = |k: &str| std::env::var(k).unwrap_or_else(|_| panic!("set {k}"));
    let client = Client::builder(session, env("VAEGRAM_API_ID").parse().unwrap(), env("VAEGRAM_API_HASH"));
    Some(client.connect().await.unwrap())
}

#[tokio::test]
#[ignore = "posts, replies to and deletes a story with a logged-in user session"]
async fn user_replies_to_a_story() {
    let Ok(photo) = std::env::var("VAEGRAM_STORY_PHOTO") else {
        eprintln!("skipped: set VAEGRAM_STORY_PHOTO (a JPEG path) and VAEGRAM_USER_SESSION");
        return;
    };
    let Some(user) = user_client().await else { return };
    let me = user.get_me().await.unwrap();
    let only_me = StoriesPrivacyRules::SelectedUsers(vec![me.id]);
    let story = user.send_story(me.id, InputStoryContent::photo(photo.as_str())).privacy(only_me).await.unwrap();

    let reply = story.reply("hi").await;
    user.delete_stories(me.id, &[story.id]).await.unwrap();
    let reply = reply.unwrap();
    assert_eq!(reply.chat.id, me.id);
    user.delete_messages("me", &[reply.id]).await.unwrap();
}
