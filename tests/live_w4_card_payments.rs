// Run with `cargo test --test live_w4_card_payments -- --ignored`; see tests/support/mod.rs for the env.
// Reading is live-unverified; paying a card form is never exercised: a provider token needs the provider's own API.

#[allow(dead_code)] // this binary drives only the user half of the kit
mod support;

use vaegram::enums::PaymentFormType;
use vaegram::types::{InvoiceRef, OrderInfo};

fn card_invoice() -> Option<InvoiceRef> {
    let link = std::env::var("VAEGRAM_CARD_INVOICE_LINK").ok();
    if link.is_none() {
        eprintln!("skipped: set VAEGRAM_CARD_INVOICE_LINK (a card invoice link)");
    }
    link.map(InvoiceRef::Link)
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a card invoice link"]
async fn card_form_reads_requirements() {
    let Some(invoice) = card_invoice() else { return };
    let Some(kit) = support::Kit::from_env().await else { return };
    let form = kit.user.client.get_payment_form(invoice).await.unwrap();
    assert_eq!(form.kind, PaymentFormType::Regular);
    assert!(form.url.is_some() && form.payment_provider.is_some());
    assert!(form.total_amount > 0);
}

#[tokio::test]
#[ignore = "uses a logged-in user session and a card invoice link"]
async fn validate_order_info_returns_an_id() {
    let Some(invoice) = card_invoice() else { return };
    let Some(kit) = support::Kit::from_env().await else { return };
    let client = kit.user.client;
    let form = client.get_payment_form(invoice).await.unwrap();
    let info = OrderInfo {
        name: form.is_name_requested.then(|| "Test Buyer".to_owned()),
        phone_number: form.is_phone_requested.then(|| "+10000000000".to_owned()),
        email: form.is_email_requested.then(|| "buyer@example.com".to_owned()),
        shipping_address: None,
    };
    if form.is_shipping_address_requested {
        eprintln!("skipped: the invoice asks for a shipping address");
        return;
    }
    let validated = client.validate_order_info(&form, info).await.unwrap();
    assert!(validated.id.is_some() || !form.is_flexible);
}

#[tokio::test]
#[ignore = "uses a logged-in user session"]
async fn saved_order_info_reads() {
    let Some(kit) = support::Kit::from_env().await else { return };
    kit.user.client.get_saved_order_info().await.unwrap();
}
