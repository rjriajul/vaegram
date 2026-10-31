// Run with `cargo test --test live_gifts -- --ignored`; see tests/support/mod.rs for the env.

#[allow(dead_code)] // this binary drives only the user half of the kit
mod support;

use vaegram::Client;
use vaegram::raw::{base, functions, types};
use vaegram::types::{GiftAuctionState, GiftAuctionStatus, InvoiceRef, OwnedGift, OwnedGifts};

async fn user() -> Option<Client> {
    Some(support::Kit::from_env().await?.user.client)
}

fn invoice_link() -> Option<String> {
    let link = std::env::var("VAEGRAM_STARS_INVOICE").ok();
    if link.is_none() {
        eprintln!("skipped: set VAEGRAM_STARS_INVOICE (a Stars invoice link)");
    }
    link
}

fn may_spend_stars() -> bool {
    let allowed = std::env::var("VAEGRAM_SPEND_STARS").is_ok_and(|v| v == "1");
    if !allowed {
        eprintln!("skipped: set VAEGRAM_SPEND_STARS=1 to let this test spend real Stars");
    }
    allowed
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a Stars invoice link"]
async fn user_reads_a_stars_invoice_without_paying() {
    let Some(link) = invoice_link() else { return };
    let Some(user) = user().await else { return };
    let before = user.get_my_star_balance().await.unwrap();
    let form = user.get_payment_form(InvoiceRef::Link(link)).await.unwrap();
    assert_eq!(form.currency, "XTR");
    assert!(form.total_amount > 0);
    assert_eq!(user.get_my_star_balance().await.unwrap(), before, "reading a form must not pay it");
}

#[tokio::test]
#[ignore = "spends real Stars: needs VAEGRAM_SPEND_STARS=1 and VAEGRAM_STARS_INVOICE"]
async fn user_pays_a_stars_invoice() {
    if !may_spend_stars() {
        return;
    }
    let Some(link) = invoice_link() else { return };
    let Some(user) = user().await else { return };
    let before = user.get_my_star_balance().await.unwrap();
    let form = user.get_payment_form(InvoiceRef::Link(link)).await.unwrap();
    let total = form.total_amount;
    if before.amount < total {
        eprintln!("skipped: the balance ({}) is below the invoice total ({total})", before.amount);
        return;
    }
    user.send_payment_form(form).await.unwrap();
    assert_eq!(user.get_my_star_balance().await.unwrap().amount, before.amount - total);
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_reads_stars_and_gift_counts() {
    let Some(user) = user().await else { return };
    user.get_ton_balance().await.unwrap();
    let count = user.get_chat_gifts_count("me").await.unwrap();
    assert_eq!(count, user.get_chat_gifts("me").limit(1).await.unwrap().total_count);

    let gifts = user.get_available_gifts().await.unwrap();
    let Some(gift) = gifts.iter().find(|g| g.upgrade_star_count.is_some()) else {
        eprintln!("skipped the upgrade checks: no gift on sale can be upgraded");
        return;
    };
    assert!(!user.get_gift_upgrade_preview(gift.id).await.unwrap().variants.models.is_empty());
    assert!(!user.get_gift_upgrade_variants(gift.id).await.unwrap().models.is_empty());

    if let Some(resale) = user.search_gifts_for_resale(gift.id).limit(1).next().await.unwrap() {
        let name = resale.gift.name;
        assert_eq!(user.get_upgraded_gift(&name).await.unwrap().name, name);
        assert!(!user.get_upgraded_gift_value_info(&name).await.unwrap().currency.is_empty());
    }
}

async fn subscriptions(user: &Client) -> Vec<types::StarsSubscription> {
    let request = functions::payments::GetStarsSubscriptions {
        missing_balance: false,
        peer: base::InputPeer::InputPeerSelf,
        offset: String::new(),
    };
    let base::payments::StarsStatus::StarsStatus(status) = user.invoke(&request).await.unwrap();
    let subscriptions = status.subscriptions.unwrap_or_default();
    subscriptions.into_iter().map(|base::StarsSubscription::StarsSubscription(s)| *s).collect()
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_cancels_and_restores_a_star_subscription() {
    let Some(user) = user().await else { return };
    let Some(active) = subscriptions(&user).await.into_iter().find(|s| !s.canceled) else {
        eprintln!("skipped: the account has no active Star subscription");
        return;
    };
    let canceled = async |id: &str| subscriptions(&user).await.into_iter().find(|s| s.id == id).map(|s| s.canceled);
    user.edit_star_subscription(&active.id, true).await.unwrap();
    assert_eq!(canceled(&active.id).await, Some(true));
    user.edit_star_subscription(&active.id, false).await.unwrap();
    assert_eq!(canceled(&active.id).await, Some(false));

    if let Some(lapsed) = subscriptions(&user).await.into_iter().find(|s| s.can_refulfill) {
        user.reuse_star_subscription(&lapsed.id).await.unwrap();
    }
}

fn owned_gift_ids(gifts: &OwnedGifts) -> Vec<String> {
    let id = |g: &OwnedGift| match g {
        OwnedGift::Regular(g) => g.owned_gift_id.clone(),
        OwnedGift::Unique(g) => g.owned_gift_id.clone(),
    };
    gifts.gifts.iter().filter_map(id).collect()
}

#[tokio::test]
#[ignore = "creates and deletes a gift collection on the user's profile"]
async fn user_manages_gift_collections() {
    let Some(user) = user().await else { return };
    let ids = owned_gift_ids(&user.get_chat_gifts("me").await.unwrap());
    let Some(first) = ids.first().map(String::as_str) else {
        eprintln!("skipped: the account owns no gift");
        return;
    };
    let first = first.to_owned();
    let created = user.create_gift_collection("me", "Live test", &[first.as_str()]).await.unwrap();
    let id = created.id;
    // The checks run in their own task so the collection is deleted even when one of them fails.
    let checks = tokio::spawn({
        let user = user.clone();
        async move {
            let listed = user.get_gift_collections("me").await.unwrap();
            assert!(listed.iter().any(|c| c.id == id));
            assert_eq!(user.set_gift_collection_name("me", id, "Live test 2").await.unwrap().name, "Live test 2");

            if let Some(second) = ids.get(1).map(String::as_str) {
                assert_eq!(user.add_collection_gifts("me", id, &[second]).await.unwrap().gift_count, 2);
                user.reorder_collection_gifts("me", id, &[second, first.as_str()]).await.unwrap();
                assert_eq!(user.remove_collection_gifts("me", id, &[second]).await.unwrap().gift_count, 1);
            }
            let inside = user.get_chat_gifts("me").collection(id).await.unwrap();
            assert_eq!(owned_gift_ids(&inside), [first]);

            // Only the test collection moves, so the others keep their order once it is deleted.
            let order: Vec<i32> = std::iter::once(id).chain(listed.iter().map(|c| c.id).filter(|&c| c != id)).collect();
            user.reorder_gift_collections("me", &order).await.unwrap();
        }
    })
    .await;
    user.delete_gift_collection("me", id).await.unwrap();
    if let Err(failed) = checks {
        std::panic::resume_unwind(failed.into_panic());
    }
    assert!(!user.get_gift_collections("me").await.unwrap().iter().any(|c| c.id == id));
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn user_reads_crafting_and_auctions() {
    let Some(user) = user().await else { return };
    let gifts = user.get_available_gifts().await.unwrap();
    match gifts.iter().find(|g| g.upgrade_star_count.is_some()) {
        Some(gift) => {
            user.get_gifts_for_crafting(gift.id).limit(10).await.unwrap();
        }
        None => eprintln!("skipped the crafting check: no gift on sale can be upgraded"),
    }
    match gifts.iter().find(|g| g.is_auction) {
        Some(gift) => assert_eq!(user.get_gift_auction_state(gift.id).await.unwrap().gift.id, gift.id),
        None => eprintln!("skipped the auction check: no gift is sold by auction"),
    }
}

#[tokio::test]
#[ignore = "spends real Stars: needs VAEGRAM_SPEND_STARS=1 and VAEGRAM_AUCTION_GIFT_ID"]
async fn user_bids_in_a_gift_auction() {
    if !may_spend_stars() {
        return;
    }
    let Some(gift_id) = std::env::var("VAEGRAM_AUCTION_GIFT_ID").ok().and_then(|id| id.parse().ok()) else {
        eprintln!("skipped: set VAEGRAM_AUCTION_GIFT_ID (the id of a gift in an active auction)");
        return;
    };
    let Some(user) = user().await else { return };
    let GiftAuctionStatus::Active(auction) = user.get_gift_auction_state(gift_id).await.unwrap().status else {
        eprintln!("skipped: the auction is not running");
        return;
    };
    let bid = auction.min_bid_star_count;
    user.place_gift_auction_bid(gift_id, bid).await.unwrap();
    let my_bid = |state: GiftAuctionState| state.my_bid.map(|b| b.star_count);
    assert_eq!(my_bid(user.get_gift_auction_state(gift_id).await.unwrap()), Some(bid));
    user.increase_gift_auction_bid(gift_id, bid + 1).await.unwrap();
    assert_eq!(my_bid(user.get_gift_auction_state(gift_id).await.unwrap()), Some(bid + 1));
}
