#[allow(dead_code)]
mod support;

use vaegram::Client;
use vaegram::enums::{StickerType, StoriesPrivacyRules};
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

#[tokio::test]
#[ignore = "reorders and restores the user's installed sticker sets with a logged-in user session"]
async fn user_reorders_sticker_sets_and_restores_them() {
    let Some(user) = user_client().await else { return };
    assert!(!user.get_suggested_sticker_set_name("vaegram test").await.unwrap().is_empty());
    user.get_owned_sticker_sets().limit(10).await.unwrap();

    let ids = |sets: Vec<vaegram::types::StickerSet>| sets.into_iter().map(|s| s.id).collect::<Vec<_>>();
    let original = ids(user.get_installed_sticker_sets(StickerType::Regular).await.unwrap());
    if original.len() < 2 {
        eprintln!("skipped the reorder: it needs two installed sticker sets");
        return;
    }
    let reversed: Vec<i64> = original.iter().rev().copied().collect();
    user.reorder_installed_sticker_sets(StickerType::Regular, &reversed).await.unwrap();
    assert_eq!(ids(user.get_installed_sticker_sets(StickerType::Regular).await.unwrap()), reversed);
    user.reorder_installed_sticker_sets(StickerType::Regular, &original).await.unwrap();
    assert_eq!(ids(user.get_installed_sticker_sets(StickerType::Regular).await.unwrap()), original);
}
