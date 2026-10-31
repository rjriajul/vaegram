#[allow(dead_code)]
mod support;

use vaegram::Client;
use vaegram::enums::StoriesPrivacyRules;
use vaegram::raw::base;
use vaegram::types::InputStoryContent;

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
#[ignore = "posts, forwards, copies and deletes stories with a logged-in user session"]
async fn user_checks_forwards_and_copies_a_story() {
    let Ok(photo) = std::env::var("VAEGRAM_STORY_PHOTO") else {
        eprintln!("skipped: set VAEGRAM_STORY_PHOTO (a JPEG path) and VAEGRAM_USER_SESSION");
        return;
    };
    let Some(user) = user_client().await else { return };
    let base::User::User(me) = user.get_me().await.unwrap() else { panic!("get_me returned an empty user") };
    assert!(user.can_post_stories(me.id).await.unwrap() > 0);

    let only_me = StoriesPrivacyRules::SelectedUsers(vec![me.id]);
    let story =
        user.send_story(me.id, InputStoryContent::photo(photo.as_str())).privacy(only_me.clone()).await.unwrap();

    let forwarded = user.forward_story("me", me.id, story.id).await.unwrap();
    assert_eq!(forwarded.chat.id, me.id);
    user.delete_messages("me", &[forwarded.id]).await.unwrap();

    let copy = user.copy_story(me.id, me.id, story.id).caption("copy").privacy(only_me).await.unwrap();
    assert_ne!(copy.id, story.id);
    assert_eq!(copy.caption.as_deref(), Some("copy"));
    assert!(copy.media.is_some());

    user.delete_stories(me.id, &[story.id, copy.id]).await.unwrap();
}
