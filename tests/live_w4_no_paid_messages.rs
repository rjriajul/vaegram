// Run with `cargo test --test live_w4_no_paid_messages -- --ignored`; see tests/support/mod.rs for the env.
// Unverified against a live account. Only exports with `revoke` off, so no existing link is cancelled.

#[allow(dead_code)] // this binary drives only the user half of the kit
mod support;

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn export_no_paid_messages_token_returns_link() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let token = kit.user.client.export_no_paid_messages_token().await.unwrap();
    assert!(!token.token.is_empty());
    assert!(token.link.contains(&token.token));
}
