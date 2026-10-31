use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, types};
use crate::types::PeerRef;

impl Client {
    /// `gift` is the gift's name or its `t.me/nft/<name>` link; nothing is paid above `star_count`.
    pub async fn send_resold_gift(
        &self,
        gift: &str,
        new_owner_chat_id: impl Into<PeerRef>,
        star_count: i64,
    ) -> Result<()> {
        let slug = gift_slug(gift)?;
        if star_count <= 0 {
            return Err(Error::InvalidArgument("star_count must be positive"));
        }
        let invoice = base::InputInvoice::InputInvoiceStarGiftResale(Box::new(types::InputInvoiceStarGiftResale {
            ton: false,
            show_name: false,
            slug: slug.into(),
            to_id: self.resolve_peer(new_owner_chat_id).await?,
            message: None,
        }));
        self.pay_in_stars_up_to(invoice, star_count).await
    }
}

pub(super) fn gift_slug(gift: &str) -> Result<&str> {
    let slug = gift.trim().trim_end_matches('/').rsplit('/').next().unwrap_or_default();
    if slug.is_empty() || slug == "nft" {
        return Err(Error::InvalidArgument("the gift link has no name"));
    }
    Ok(slug)
}

#[cfg(test)]
mod tests {
    use super::gift_slug;
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Identifiable, Serializable, base, functions, types};

    fn star_gift_form(price: i64) -> Vec<u8> {
        star_gift_form_in("XTR", price)
    }

    fn star_gift_form_in(currency: &str, price: i64) -> Vec<u8> {
        let price = types::LabeledPrice { label: "Gift".into(), amount: price };
        let price = base::LabeledPrice::LabeledPrice(Box::new(price));
        let invoice = types::Invoice {
            test: false,
            name_requested: false,
            phone_requested: false,
            email_requested: false,
            shipping_address_requested: false,
            flexible: false,
            phone_to_provider: false,
            email_to_provider: false,
            recurring: false,
            currency: currency.into(),
            prices: vec![price],
            max_tip_amount: None,
            suggested_tip_amounts: None,
            terms_url: None,
            subscription_period: None,
        };
        let invoice = base::Invoice::Invoice(Box::new(invoice));
        let form = types::payments::PaymentFormStarGift { form_id: 7, invoice };
        base::payments::PaymentForm::PaymentFormStarGift(Box::new(form)).to_bytes()
    }

    fn client(replies: Vec<Vec<u8>>) -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (client, sent) = Client::scripted(replies.into_iter().map(Ok).collect());
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 5, access_hash: 6 }));
        client.0.peers.lock().unwrap().insert(5, peer);
        (client, sent)
    }

    fn constructor(body: &[u8]) -> u32 {
        u32::from_le_bytes(body[..4].try_into().unwrap())
    }

    #[tokio::test]
    async fn pays_up_to_the_agreed_price() {
        let paid = base::payments::PaymentResult::PaymentResult(Box::new(types::payments::PaymentResult {
            updates: base::Updates::UpdatesTooLong,
        }));
        let (client, mut sent) = client(vec![star_gift_form(900), paid.to_bytes()]);
        client.send_resold_gift("https://t.me/nft/NekoHelmet-9215", 5, 900).await.unwrap();
        assert_eq!(constructor(&sent.try_recv().unwrap()), functions::payments::GetPaymentForm::CONSTRUCTOR_ID);
        assert_eq!(constructor(&sent.try_recv().unwrap()), functions::payments::SendStarsForm::CONSTRUCTOR_ID);
        assert!(matches!(client.0.events.take_nested(), Some(base::Updates::UpdatesTooLong)));
    }

    #[tokio::test]
    async fn a_link_without_a_name_is_refused_without_a_request() {
        let (client, mut sent) = client(Vec::new());
        for gift in ["", "  ", "https://t.me/nft/", "t.me/nft"] {
            assert!(matches!(client.send_resold_gift(gift, 5, 900).await, Err(Error::InvalidArgument(_))), "{gift}");
        }
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_price_of_nothing_is_refused_without_a_request() {
        let (client, mut sent) = client(Vec::new());
        for stars in [0, -5] {
            let refused = client.send_resold_gift("NekoHelmet-9215", 5, stars).await;
            assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{stars}");
        }
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_price_that_is_not_in_stars_is_not_paid() {
        let (client, mut sent) = client(vec![star_gift_form_in("TON", 1)]);
        assert!(matches!(client.send_resold_gift("NekoHelmet-9215", 5, 900).await, Err(Error::Unexpected(_))));
        assert_eq!(constructor(&sent.try_recv().unwrap()), functions::payments::GetPaymentForm::CONSTRUCTOR_ID);
        assert!(sent.try_recv().is_err(), "a price that isn't in Stars must not be paid");
    }

    #[tokio::test]
    async fn refuses_a_raised_price() {
        let (client, mut sent) = client(vec![star_gift_form(1000)]);
        assert!(client.send_resold_gift("NekoHelmet-9215", 5, 900).await.is_err());
        assert_eq!(constructor(&sent.try_recv().unwrap()), functions::payments::GetPaymentForm::CONSTRUCTOR_ID);
        assert!(sent.try_recv().is_err(), "nothing may be paid above star_count");
    }

    #[test]
    fn takes_the_name_from_links() {
        assert_eq!(gift_slug("https://t.me/nft/NekoHelmet-9215").unwrap(), "NekoHelmet-9215");
        assert_eq!(gift_slug("t.me/nft/NekoHelmet-9215/").unwrap(), "NekoHelmet-9215");
        assert_eq!(gift_slug(" NekoHelmet-9215 ").unwrap(), "NekoHelmet-9215");
    }
}
