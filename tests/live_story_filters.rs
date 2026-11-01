#[allow(dead_code)]
mod support;

use std::time::Duration;

use support::Kit;
use tokio::sync::mpsc;
use vaegram::enums::StoriesPrivacyRules;
use vaegram::filters;
use vaegram::raw::base;
use vaegram::types::InputStoryContent;

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
    let base::User::User(me) = user.get_me().await.unwrap() else { panic!("get_me returned an empty user") };
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
