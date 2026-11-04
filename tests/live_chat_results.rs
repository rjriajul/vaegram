// Run with `cargo test --test live_chat_results -- --ignored`; see tests/support/mod.rs for the env.
// Live behaviour is unverified: these tests have not been run.

#[allow(dead_code)] // this binary drives only the user half of the kit
mod support;

#[tokio::test]
#[ignore]
async fn search_contacts_splits_results() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let found = kit.user.client.search_contacts("telegram", 5).await.unwrap();
    assert!(found.my_results.len() + found.global_results.len() > 0);
}

#[tokio::test]
#[ignore]
async fn invite_preview() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let Ok(link) = std::env::var("VAEGRAM_INVITE_LINK") else {
        eprintln!("skipped: set VAEGRAM_INVITE_LINK (an invite link)");
        return;
    };
    let preview = kit.user.client.get_chat_preview(&link).await.unwrap();
    assert!(!preview.title.is_empty());
}
