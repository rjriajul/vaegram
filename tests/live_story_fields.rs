// Run with `cargo test --test live_story_fields -- --ignored`; see tests/support/mod.rs for the env.

#[allow(dead_code)] // this binary drives only the user half of the kit
mod support;

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn own_stories_carry_privacy() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let user = kit.user.client;
    let mut pages = user.get_pinned_stories("me").limit(20);
    let mut seen = 0;
    while let Some(story) = pages.next().await.unwrap() {
        if story.outgoing {
            seen += 1;
            assert!(story.privacy.is_some(), "own story {} came back without privacy rules", story.id);
        }
    }
    if seen == 0 {
        eprintln!("skipped: the account has no pinned stories of its own");
    }
}
