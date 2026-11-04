// Run with `cargo test --test live_small_methods -- --ignored`; see tests/support/mod.rs for the env.
// Live behaviour is unverified: this test has not been run. The topic-unread and protected-content calls
// need a channel's direct-messages admin account and a second user, so they have no live test.

#[allow(dead_code)] // this binary drives only the user half of the kit
mod support;

#[tokio::test]
#[ignore]
async fn nearby_chats_answers() {
    let Some(kit) = support::Kit::from_env().await else { return };
    match kit.user.client.get_nearby_chats(48.8584, 2.2945).await {
        Ok(chats) => eprintln!("{} nearby chats", chats.len()),
        Err(error) => eprintln!("rejected: {error}"),
    }
}
