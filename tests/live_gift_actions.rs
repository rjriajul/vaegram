// Run with `cargo test --test live_gift_actions -- --ignored`; see tests/support/mod.rs for the env.
// Unverified against a live account. Reads only: nothing is bought, sent or transferred.

#[allow(dead_code)] // this binary drives only the user half of the kit
mod support;

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn resale_results_carry_resale_parameters() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let client = kit.user.client;
    let gifts = client.get_available_gifts().await.unwrap();
    let Some(gift) = gifts.iter().find(|g| g.available_resale_count.is_some_and(|n| n > 0)) else {
        eprintln!("skipped: no gift is listed for resale");
        return;
    };
    let Some(listed) = client.search_gifts_for_resale(gift.id).limit(1).next().await.unwrap() else { return };
    let price = listed.resale_parameters.expect("a resale listing has a price");
    assert!(price.star_count.is_some() || price.toncoin_nano_count.is_some());
}
