#[allow(dead_code)]
mod support;

use std::time::Duration;

use support::Kit;
use tokio::sync::mpsc;
use vaegram::enums::StoriesPrivacyRules;
use vaegram::filters;
use vaegram::types::{InlineKeyboardButton, InlineKeyboardMarkup, InputStoryContent};

const WAIT: Duration = Duration::from_secs(30);

#[tokio::test]
#[ignore = "uses a bot token and a logged-in user session; posts and deletes a story"]
async fn on_story_sees_a_posted_story() {
    let Ok(photo) = std::env::var("VAEGRAM_STORY_PHOTO") else {
        eprintln!("skipped: set VAEGRAM_STORY_PHOTO (a JPEG path), VAEGRAM_USER_SESSION and VAEGRAM_BOT_TOKEN");
        return;
    };
    let Some(kit) = Kit::from_env().await else { return };
    let user = &kit.user.client;
    let me = user.get_me().await.unwrap();
    let (tx, mut seen) = mpsc::unbounded_channel();
    user.on_story(filters::chat(me.id), move |story| {
        let _ = tx.send(story.id);
        async { Ok(()) }
    });

    let only_me = StoriesPrivacyRules::SelectedUsers(vec![me.id]);
    let story = user.send_story(me.id, InputStoryContent::photo(photo.as_str())).privacy(only_me).await.unwrap();
    let arrived = tokio::time::timeout(WAIT, async {
        while let Some(id) = seen.recv().await {
            if id == story.id {
                return true;
            }
        }
        false
    })
    .await;
    user.delete_stories(me.id, &[story.id]).await.unwrap();
    assert_eq!(arrived, Ok(true), "no updateStory arrived for the posted story");
}

#[tokio::test]
#[ignore = "uses a bot token and a logged-in user session"]
async fn a_bot_keyboard_matches_inline_keyboard() {
    let Some(kit) = Kit::from_env().await else { return };
    kit.bot.on_message(filters::private() & filters::command("keyboard"), |m| async move {
        let keyboard = InlineKeyboardMarkup::new([vec![InlineKeyboardButton::callback("Tap", "tap")]]);
        m.reply("Keyboard").reply_markup(keyboard).await?;
        Ok(())
    });
    let bot = kit.bot.get_me().await.unwrap();
    let (tx, mut matched) = mpsc::unbounded_channel();
    kit.user
        .client
        .on_message(filters::inline_keyboard() & filters::user(bot.id), move |m| {
            let _ = tx.send(m.id);
            async { Ok(()) }
        })
        .group(1);

    let reply = kit.user.send_and_wait_reply(&kit.bot, "/keyboard").await.unwrap();
    let matched = tokio::time::timeout(WAIT, matched.recv()).await;
    kit.user.client.delete_messages(reply.chat.id, &[reply.id]).await.unwrap();
    assert_eq!(matched, Ok(Some(reply.id)));
}
