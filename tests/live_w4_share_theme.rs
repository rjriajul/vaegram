// Run with `cargo test --test live_w4_share_theme -- --ignored`; see tests/support/mod.rs for the env.
// Unverified against a live account.

#[allow(dead_code)] // this binary drives only the user half of the kit
mod support;

#[tokio::test]
#[ignore = "uses a logged-in user session; changes the share theme of the account's own profile, then resets it"]
async fn share_theme_round_trip() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let client = kit.user.client;
    let backdrops = client.get_share_theme_backdrops().await.unwrap();
    let Some(backdrop) = backdrops.first() else {
        eprintln!("skipped: no share theme backdrops");
        return;
    };
    // Resetting at the end would wipe a custom theme, so only an account on the default theme is used.
    let current = client.get_chat("me").await.unwrap().full_info.and_then(|i| i.share_theme);
    if current.is_some() {
        eprintln!("skipped: the account already has a custom share theme");
        return;
    }
    client.set_share_theme("me").backdrop_id(backdrop.id).await.unwrap();
    let theme = client.get_chat("me").await.unwrap().full_info.and_then(|i| i.share_theme);
    assert_eq!(theme.and_then(|t| t.backdrop_id), Some(backdrop.id));
    client.set_share_theme("me").await.unwrap();
}
