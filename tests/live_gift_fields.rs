// Run with `cargo test --test live_gift_fields -- --ignored`; see tests/support/mod.rs for the env.
// Unverified against a live account.

#[allow(dead_code)] // this binary drives only the user half of the kit
mod support;

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn available_gifts_read_limits() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let gifts = kit.user.client.get_available_gifts().await.unwrap();
    assert!(gifts.iter().any(|g| g.title.is_some() || g.convert_star_count > 0));
    for gift in gifts.iter().filter(|g| g.is_sold_out) {
        assert!(gift.is_limited, "a sold-out gift {} must be a limited one", gift.id);
    }
}
