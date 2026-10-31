use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::PaymentForm;

impl Client {
    /// Pays a form priced in Stars. A timed-out payment may have gone through: check the balance before retrying.
    pub async fn send_payment_form(&self, form: PaymentForm) -> Result<()> {
        if form.currency != "XTR" {
            return Err(Error::InvalidArgument("only forms priced in Stars can be paid"));
        }
        let request = functions::payments::SendStarsForm { form_id: form.id, invoice: form.invoice };
        match self.invoke(&request).await? {
            base::payments::PaymentResult::PaymentResult(paid) => {
                self.feed_updates(paid.updates);
                Ok(())
            }
            base::payments::PaymentResult::PaymentVerificationNeeded(_) => {
                Err(Error::Unexpected("verification asked for a Stars payment"))
            }
        }
    }

    pub(crate) async fn pay_in_stars_up_to(&self, invoice: base::InputInvoice, star_count: i64) -> Result<()> {
        if star_count <= 0 {
            return Err(Error::InvalidArgument("star_count must be positive"));
        }
        let form = self.payment_form(invoice).await?;
        if form.currency != "XTR" {
            return Err(Error::Unexpected("the payment form is not priced in Stars"));
        }
        // The price can change between the caller's look and checkout.
        if form.total_amount > star_count {
            return Err(Error::InvalidArgument("the price is above star_count"));
        }
        self.send_payment_form(form).await
    }
}

#[cfg(test)]
pub(super) mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::InvoiceRef;

    pub(in crate::methods::payments) type Sent = tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>;

    pub(in crate::methods::payments) fn client(replies: Vec<Vec<u8>>) -> (Client, Sent) {
        let (client, sent) = Client::scripted(replies.into_iter().map(Ok).collect());
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 5, access_hash: 6 }));
        client.0.peers.lock().unwrap().insert(5, peer);
        (client, sent)
    }

    pub(in crate::methods::payments) fn invoice(currency: &str, prices: &[i64]) -> base::Invoice {
        let prices = prices
            .iter()
            .map(|&amount| {
                base::LabeledPrice::LabeledPrice(Box::new(types::LabeledPrice { label: "Item".into(), amount }))
            })
            .collect();
        base::Invoice::Invoice(Box::new(types::Invoice {
            test: true,
            name_requested: false,
            phone_requested: false,
            email_requested: false,
            shipping_address_requested: false,
            flexible: false,
            phone_to_provider: false,
            email_to_provider: false,
            recurring: false,
            currency: currency.into(),
            prices,
            max_tip_amount: None,
            suggested_tip_amounts: None,
            terms_url: None,
            subscription_period: Some(2592000),
        }))
    }

    // Only access_hash and first_name are set.
    fn shop_bot() -> base::User {
        let mut buf = Vec::new();
        for word in [0b11u32, 0] {
            word.serialize(&mut buf);
        }
        8i64.serialize(&mut buf);
        80i64.serialize(&mut buf);
        String::from("Shop").serialize(&mut buf);
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    /// A bot's form with id 7, sold by user 8 ("Shop").
    pub(in crate::methods::payments) fn stars_form(currency: &str, prices: &[i64]) -> Vec<u8> {
        let form = types::payments::PaymentFormStars {
            form_id: 7,
            bot_id: 8,
            title: "Pro".into(),
            description: "A month of Pro".into(),
            photo: None,
            invoice: invoice(currency, prices),
            users: vec![shop_bot()],
        };
        base::payments::PaymentForm::PaymentFormStars(Box::new(form)).to_bytes()
    }

    /// A star gift's form with id 7.
    pub(in crate::methods::payments) fn gift_form(currency: &str, price: i64) -> Vec<u8> {
        let form = types::payments::PaymentFormStarGift { form_id: 7, invoice: invoice(currency, &[price]) };
        base::payments::PaymentForm::PaymentFormStarGift(Box::new(form)).to_bytes()
    }

    pub(in crate::methods::payments) fn paid() -> Vec<u8> {
        let paid = types::payments::PaymentResult { updates: base::Updates::UpdatesTooLong };
        base::payments::PaymentResult::PaymentResult(Box::new(paid)).to_bytes()
    }

    /// The two requests a payment of `invoice` sends: the form, then form 7 paid.
    pub(in crate::methods::payments) fn payment(invoice: base::InputInvoice) -> [Vec<u8>; 2] {
        let form = functions::payments::GetPaymentForm { invoice: invoice.clone(), theme_params: None };
        [form.to_bytes(), functions::payments::SendStarsForm { form_id: 7, invoice }.to_bytes()]
    }

    fn slug() -> base::InputInvoice {
        base::InputInvoice::InputInvoiceSlug(Box::new(types::InputInvoiceSlug { slug: "abc".into() }))
    }

    #[tokio::test]
    async fn send_payment_form_pays_exactly_the_given_form() {
        let (client, mut sent) = client(vec![stars_form("XTR", &[900]), paid()]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        client.send_payment_form(form).await.unwrap();
        let [get, pay] = payment(slug());
        assert_eq!(sent.try_recv().unwrap(), get);
        assert_eq!(sent.try_recv().unwrap(), pay);
        assert!(sent.try_recv().is_err());
        assert!(matches!(client.0.events.take_nested(), Some(base::Updates::UpdatesTooLong)));
    }

    #[tokio::test]
    async fn send_payment_form_refuses_a_card_form_without_a_request() {
        let (client, mut sent) = client(vec![stars_form("USD", &[900])]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        assert!(matches!(client.send_payment_form(form).await, Err(Error::InvalidArgument(_))));
        assert_eq!(sent.try_recv().unwrap(), payment(slug())[0]);
        assert!(sent.try_recv().is_err(), "a card form must not be paid");
    }

    #[tokio::test]
    async fn verification_for_a_stars_form_is_an_error() {
        let verify = types::payments::PaymentVerificationNeeded { url: "https://t.me".into() };
        let verify = base::payments::PaymentResult::PaymentVerificationNeeded(Box::new(verify));
        let (client, _sent) = client(vec![stars_form("XTR", &[900]), verify.to_bytes()]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        assert!(matches!(client.send_payment_form(form).await, Err(Error::Unexpected(_))));
    }

    #[tokio::test]
    async fn a_refused_payment_returns_the_telegram_error() {
        let replies = vec![Ok(stars_form("XTR", &[900])), Err(Error::Rpc(RpcError::new(400, "FORM_EXPIRED")))];
        let (client, _sent) = Client::scripted(replies);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        let result = client.send_payment_form(form).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "FORM_EXPIRED"), "{result:?}");
    }

    #[tokio::test]
    async fn pays_up_to_the_cap() {
        let (client, mut sent) = client(vec![gift_form("XTR", 900), paid()]);
        client.pay_in_stars_up_to(slug(), 900).await.unwrap();
        let [get, pay] = payment(slug());
        assert_eq!(sent.try_recv().unwrap(), get);
        assert_eq!(sent.try_recv().unwrap(), pay);
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn refuses_a_price_above_the_cap() {
        let (client, mut sent) = client(vec![gift_form("XTR", 1000)]);
        assert!(matches!(client.pay_in_stars_up_to(slug(), 900).await, Err(Error::InvalidArgument(_))));
        assert_eq!(sent.try_recv().unwrap(), payment(slug())[0]);
        assert!(sent.try_recv().is_err(), "nothing may be paid above the cap");
    }

    #[tokio::test]
    async fn refuses_an_overflowing_price() {
        let (client, mut sent) = client(vec![stars_form("XTR", &[i64::MAX, i64::MAX])]);
        assert!(client.pay_in_stars_up_to(slug(), i64::MAX - 1).await.is_err());
        assert_eq!(sent.try_recv().unwrap(), payment(slug())[0]);
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn refuses_a_form_not_in_stars() {
        let (client, mut sent) = client(vec![gift_form("TON", 1)]);
        assert!(matches!(client.pay_in_stars_up_to(slug(), 900).await, Err(Error::Unexpected(_))));
        assert_eq!(sent.try_recv().unwrap(), payment(slug())[0]);
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn refuses_a_non_positive_cap_without_a_request() {
        let (client, mut sent) = client(Vec::new());
        for cap in [0, -1] {
            assert!(matches!(client.pay_in_stars_up_to(slug(), cap).await, Err(Error::InvalidArgument(_))), "{cap}");
        }
        assert!(sent.try_recv().is_err());
    }
}
