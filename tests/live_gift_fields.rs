// Run with `cargo test --test live_gift_fields -- --ignored`; see tests/support/mod.rs for the env.

#[allow(dead_code)] // this binary drives only the user half of the kit
mod support;

use vaegram::raw::{base, functions};

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn available_gifts_read_limits() {
    let Some(kit) = support::Kit::from_env().await else { return };
    let client = &kit.user.client;
    // What Telegram lists, counted from the raw answer, tells an empty shop apart from gifts lost while parsing.
    let base::payments::StarGifts::StarGifts(shop) =
        client.invoke(&functions::payments::GetStarGifts { hash: 0 }).await.unwrap()
    else {
        panic!("payments.getStarGifts returned no gift list")
    };
    let listed = shop.gifts.iter().filter(|g| matches!(g, base::StarGift::StarGift(_))).count();
    if listed == 0 {
        eprintln!("skipped: Telegram's gift shop lists no regular gifts for this account");
        return;
    }

    let gifts = client.get_available_gifts().await.unwrap();
    assert_eq!(gifts.len(), listed, "gifts were dropped while parsing");
    assert!(
        gifts.iter().any(|g| g.title.is_some() || g.convert_star_count > 0),
        "no gift has a title or a conversion price: {:?}",
        gifts.iter().map(|g| (g.id, &g.title, g.convert_star_count)).collect::<Vec<_>>()
    );
    for gift in gifts.iter().filter(|g| g.is_sold_out) {
        assert!(gift.is_limited, "a sold-out gift {} must be a limited one", gift.id);
    }
}
