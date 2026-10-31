// Run with `cargo test --test live_gifts -- --ignored`; see tests/support/mod.rs for the env.

#[allow(dead_code)] // this binary drives only the user half of the kit
mod support;

use vaegram::Client;
use vaegram::types::InvoiceRef;

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
