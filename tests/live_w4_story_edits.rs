// Run with `cargo test --test live_w4_story_edits -- --ignored`; see tests/support/mod.rs for the env.

#[allow(dead_code)] // this binary drives only the user half of the kit
mod support;

use vaegram::types::{InputStoryContent, StoryArea, StoryAreaPosition, StoryAreaType};

#[tokio::test]
#[ignore = "posts a story to the account's own chat: needs VAEGRAM_POST_STORIES=1 and VAEGRAM_STORY_PHOTO"]
async fn edit_story_media_keeps_areas() {
    if std::env::var("VAEGRAM_POST_STORIES").is_ok_and(|v| v == "1") {
        let Ok(photo) = std::env::var("VAEGRAM_STORY_PHOTO") else {
            eprintln!("skipped: set VAEGRAM_STORY_PHOTO (a 1080x1920 image file)");
            return;
        };
        let Some(kit) = support::Kit::from_env().await else { return };
        let user = kit.user.client;
        let position = StoryAreaPosition::new(50.0, 50.0, 0.0, 0.0);
        let link = StoryArea::new(position, StoryAreaType::Link { url: "https://example.com".into() });
        let story = user.send_story("me", InputStoryContent::photo(photo.as_str())).media_areas([link]).await.unwrap();
        let edited = user.edit_story_media("me", story.id, InputStoryContent::photo(photo.as_str())).await.unwrap();
        let stories = user.get_stories("me", &[edited.id]).await.unwrap();
        let kept = stories.first().map(|s| s.media_areas.len());
        user.delete_stories("me", &[story.id]).await.unwrap();
        assert_eq!(kept, Some(1), "an edit that sets no areas must keep the existing ones");
    } else {
        eprintln!("skipped: set VAEGRAM_POST_STORIES=1 to let this test post a story");
    }
}
