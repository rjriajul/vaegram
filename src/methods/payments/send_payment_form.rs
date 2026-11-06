use crate::Client;
use crate::errors::{Error, Result};
use crate::raw::{base, functions};
use crate::types::{InputCredentials, PaymentForm, PaymentResult, StarAmount};

/// Request built by [`Client::send_payment_form`].
///
/// A Stars or Toncoin form takes no credentials, order info, shipping or tip, and setting any of them fails with
/// `Error::InvalidArgument`. A card form fails with `Error::InvalidArgument` without
/// [`credentials`](SendPaymentForm::credentials).
///
/// A timed-out payment may have gone through, so check the balance before retrying.
#[must_use = "a request does nothing until it is awaited"]
pub struct SendPaymentForm<'a> {
    client: &'a Client,
    form: PaymentForm,
    credentials: Option<InputCredentials>,
    order_info_id: Option<String>,
    shipping_option_id: Option<String>,
    tip_amount: Option<i64>,
}

impl Client {
    /// Pays a [`PaymentForm`] from [`Client::get_payment_form`].
    ///
    /// Awaiting returns a [`PaymentResult`], which holds a URL when the provider asks for verification.
    pub fn send_payment_form(&self, form: PaymentForm) -> SendPaymentForm<'_> {
        SendPaymentForm {
            client: self,
            form,
            credentials: None,
            order_info_id: None,
            shipping_option_id: None,
            tip_amount: None,
        }
    }

    /// Pays a Stars (or, when `cap.ton`, Toncoin) form unless it costs more than `cap`.
    pub(crate) async fn pay_form_up_to(&self, invoice: base::InputInvoice, cap: StarAmount) -> Result<()> {
        if cap.amount <= 0 {
            return Err(Error::InvalidArgument("star_count must be positive"));
        }
        let form = self.payment_form(invoice).await?;
        if form.currency != if cap.ton { "TON" } else { "XTR" } {
            return Err(Error::Unexpected("the payment form is not priced in the capped currency"));
        }
        // The price can change between the caller's look and checkout.
        if form.total_amount > cap.amount {
            return Err(Error::InvalidArgument("the price is above star_count"));
        }
        match self.send_payment_form(form).await? {
            PaymentResult::Paid => Ok(()),
            _ => Err(Error::Unexpected("verification asked for a Stars payment")),
        }
    }

    pub(crate) async fn pay_in_stars_up_to(&self, invoice: base::InputInvoice, star_count: i64) -> Result<()> {
        self.pay_form_up_to(invoice, StarAmount::new(star_count)).await
    }
}

impl SendPaymentForm<'_> {
    /// How to pay a card form. Stars forms take none.
    pub fn credentials(mut self, credentials: InputCredentials) -> Self {
        self.credentials = Some(credentials);
        self
    }

    /// Attaches the order details checked by [`validate_order_info`](Client::validate_order_info), by the id
    /// it returned.
    pub fn order_info_id(mut self, id: impl Into<String>) -> Self {
        self.order_info_id = Some(id.into());
        self
    }

    /// Picks one of the shipping options [`validate_order_info`](Client::validate_order_info) returned.
    pub fn shipping_option_id(mut self, id: impl Into<String>) -> Self {
        self.shipping_option_id = Some(id.into());
        self
    }

    /// A tip in the smallest unit of the form's currency, at most [`PaymentForm::max_tip_amount`].
    pub fn tip_amount(mut self, tip_amount: i64) -> Self {
        self.tip_amount = Some(tip_amount);
        self
    }

    async fn send(self) -> Result<PaymentResult> {
        let (client, form) = (self.client, self.form);
        if form.currency == "XTR" || form.currency == "TON" {
            if self.credentials.is_some() {
                return Err(Error::InvalidArgument("a form priced in Stars or Toncoin takes no credentials"));
            }
            if self.order_info_id.is_some() || self.shipping_option_id.is_some() || self.tip_amount.is_some() {
                return Err(Error::InvalidArgument(
                    "a form priced in Stars or Toncoin takes no order info, shipping or tip",
                ));
            }
            let request = functions::payments::SendStarsForm { form_id: form.id, invoice: form.invoice };
            return finish(client, client.invoke(&request).await?);
        }
        let Some(credentials) = self.credentials else {
            return Err(Error::InvalidArgument("a card form needs credentials"));
        };
        let request = functions::payments::SendPaymentForm {
            form_id: form.id,
            invoice: form.invoice,
            requested_info_id: self.order_info_id,
            shipping_option_id: self.shipping_option_id,
            credentials: credentials.write(client).await?,
            tip_amount: self.tip_amount,
        };
        finish(client, client.invoke(&request).await?)
    }
}

fn finish(client: &Client, result: base::payments::PaymentResult) -> Result<PaymentResult> {
    Ok(match result {
        base::payments::PaymentResult::PaymentResult(paid) => {
            client.feed_updates(paid.updates);
            PaymentResult::Paid
        }
        base::payments::PaymentResult::PaymentVerificationNeeded(v) => PaymentResult::VerificationNeeded { url: v.url },
    })
}

into_future!(SendPaymentForm, PaymentResult);

#[cfg(test)]
pub(super) mod tests {
    use crate::Client;
    use crate::errors::{Error, RpcError};
    use crate::raw::{Identifiable, Serializable, base, functions, types};
    use crate::types::{InputCredentials, InvoiceRef, OrderInfo, PaymentResult, StarAmount};

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
        user(8, "Shop")
    }

    fn user(id: i64, first_name: &str) -> base::User {
        let mut buf = Vec::new();
        for word in [0b11u32, 0] {
            word.serialize(&mut buf);
        }
        id.serialize(&mut buf);
        80i64.serialize(&mut buf);
        first_name.to_owned().serialize(&mut buf);
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

    /// A card form with id 3, sold by user 8 ("Shop") through provider 9 ("Provider"); it has a saved card, saved
    /// order info and a second way to pay.
    pub(in crate::methods::payments) fn card_form(currency: &str, price: i64) -> Vec<u8> {
        let info = OrderInfo::new().name("Ann");
        let photo = types::WebDocumentNoProxy {
            url: "https://example.com/p.jpg".into(),
            size: 1,
            mime_type: "image/jpeg".into(),
            attributes: Vec::new(),
        };
        let form = types::payments::PaymentForm {
            can_save_credentials: true,
            password_missing: true,
            form_id: 3,
            bot_id: 8,
            title: "Pro".into(),
            description: "A month of Pro".into(),
            photo: Some(base::WebDocument::WebDocumentNoProxy(Box::new(photo))),
            invoice: invoice(currency, &[price]),
            provider_id: 9,
            url: "https://pay.example.com/form".into(),
            native_provider: Some("stripe".into()),
            native_params: Some(base::DataJSON::DataJSON(Box::new(types::DataJSON { data: "{\"k\":1}".into() }))),
            additional_methods: Some(vec![base::PaymentFormMethod::PaymentFormMethod(Box::new(
                types::PaymentFormMethod { url: "https://wallet.example.com".into(), title: "Wallet".into() },
            ))]),
            saved_info: Some(info.write()),
            saved_credentials: Some(vec![base::PaymentSavedCredentials::PaymentSavedCredentialsCard(Box::new(
                types::PaymentSavedCredentialsCard { id: "card-1".into(), title: "Visa *1111".into() },
            ))]),
            users: vec![shop_bot(), user(9, "Provider")],
        };
        base::payments::PaymentForm::PaymentForm(Box::new(form)).to_bytes()
    }

    /// A Stars form without a subscription period.
    pub(in crate::methods::payments) fn stars_form_without_period() -> Vec<u8> {
        let mut form = types::payments::PaymentFormStars {
            form_id: 7,
            bot_id: 8,
            title: "Pro".into(),
            description: "Once".into(),
            photo: None,
            invoice: invoice("XTR", &[5]),
            users: vec![shop_bot()],
        };
        let base::Invoice::Invoice(terms) = &mut form.invoice;
        terms.subscription_period = None;
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
    async fn card_form_without_credentials_is_refused_without_a_request() {
        let (client, mut sent) = client(vec![card_form("USD", 900)]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        assert!(matches!(client.send_payment_form(form).await, Err(Error::InvalidArgument(_))));
        assert_eq!(sent.try_recv().unwrap(), payment(slug())[0]);
        assert!(sent.try_recv().is_err(), "a card form must not be paid");
    }

    fn verify() -> Vec<u8> {
        let verify = types::payments::PaymentVerificationNeeded { url: "https://t.me".into() };
        base::payments::PaymentResult::PaymentVerificationNeeded(Box::new(verify)).to_bytes()
    }

    #[tokio::test]
    async fn verification_url_is_returned() {
        let (client, _sent) = client(vec![stars_form("XTR", &[900]), verify()]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        let result = client.send_payment_form(form).await.unwrap();
        assert_eq!(result, PaymentResult::VerificationNeeded { url: "https://t.me".into() });
        assert_eq!((result.success(), result.verification_url()), (false, Some("https://t.me")));
        assert!(PaymentResult::Paid.success() && PaymentResult::Paid.verification_url().is_none());
    }

    #[tokio::test]
    async fn verification_for_a_capped_payment_is_an_error() {
        let (client, _sent) = client(vec![gift_form("XTR", 900), verify()]);
        assert!(matches!(client.pay_in_stars_up_to(slug(), 900).await, Err(Error::Unexpected(_))));
    }

    fn json(data: &str) -> base::DataJSON {
        base::DataJSON::DataJSON(Box::new(types::DataJSON { data: data.into() }))
    }

    fn card_payment(credentials: base::InputPaymentCredentials) -> Vec<u8> {
        functions::payments::SendPaymentForm {
            form_id: 3,
            invoice: slug(),
            requested_info_id: Some("info-1".into()),
            shipping_option_id: Some("ship-1".into()),
            credentials,
            tip_amount: Some(50),
        }
        .to_bytes()
    }

    #[tokio::test]
    async fn card_form_sends_payment_form_with_credentials_info_shipping_and_tip() {
        let (client, mut sent) = client(vec![card_form("USD", 900), paid()]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        let result = client
            .send_payment_form(form)
            .credentials(InputCredentials::new("{\"token\":1}").allow_save())
            .order_info_id("info-1")
            .shipping_option_id("ship-1")
            .tip_amount(50)
            .await
            .unwrap();
        assert_eq!(result, PaymentResult::Paid);
        sent.try_recv().unwrap();
        let credentials = types::InputPaymentCredentials { save: true, data: json("{\"token\":1}") };
        let credentials = base::InputPaymentCredentials::InputPaymentCredentials(Box::new(credentials));
        assert_eq!(sent.try_recv().unwrap(), card_payment(credentials));
        assert!(sent.try_recv().is_err());
        assert!(matches!(client.0.events.take_nested(), Some(base::Updates::UpdatesTooLong)));
    }

    #[tokio::test]
    async fn wallet_credentials_are_sent_as_their_own_kind() {
        let apple = types::InputPaymentCredentialsApplePay { payment_data: json("a") };
        let google = types::InputPaymentCredentialsGooglePay { payment_token: json("g") };
        let expected = [
            (
                InputCredentials::apple_pay("a"),
                base::InputPaymentCredentials::InputPaymentCredentialsApplePay(Box::new(apple)),
            ),
            (
                InputCredentials::google_pay("g"),
                base::InputPaymentCredentials::InputPaymentCredentialsGooglePay(Box::new(google)),
            ),
        ];
        for (credentials, raw) in expected {
            let (client, mut sent) = client(vec![card_form("USD", 900), paid()]);
            let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
            client
                .send_payment_form(form)
                .credentials(credentials)
                .order_info_id("info-1")
                .shipping_option_id("ship-1")
                .tip_amount(50)
                .await
                .unwrap();
            sent.try_recv().unwrap();
            assert_eq!(sent.try_recv().unwrap(), card_payment(raw));
        }
    }

    fn password_reply() -> Vec<u8> {
        use crate::crypto::{prime, to_be_padded};
        type Algo = types::PasswordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow;
        let p = to_be_padded::<256>(&prime::dh_prime()).to_vec();
        let algo =
            base::PasswordKdfAlgo::PasswordKdfAlgoSHA256SHA256PBKDF2HMACSHA512iter100000SHA256ModPow(Box::new(Algo {
                salt1: b"server".to_vec(),
                salt2: b"salt two".to_vec(),
                g: 3,
                p,
            }));
        let g_b = num_bigint::BigUint::from(3u8).modpow(&num_bigint::BigUint::from(12345u32), &prime::dh_prime());
        let password = types::account::Password {
            has_recovery: false,
            has_secure_values: false,
            has_password: true,
            current_algo: Some(algo.clone()),
            srp_b: Some(to_be_padded::<256>(&g_b).to_vec()),
            srp_id: Some(1),
            hint: None,
            email_unconfirmed_pattern: None,
            new_algo: algo,
            new_secure_algo: base::SecurePasswordKdfAlgo::SecurePasswordKdfAlgoUnknown,
            secure_random: Vec::new(),
            pending_reset_date: None,
            login_email_pattern: None,
        };
        base::account::Password::Password(Box::new(password)).to_bytes()
    }

    #[tokio::test]
    async fn saved_credentials_send_get_password_then_tmp_password() {
        let tmp = types::account::TmpPassword { tmp_password: vec![7; 32], valid_until: 99 };
        let tmp = base::account::TmpPassword::TmpPassword(Box::new(tmp)).to_bytes();
        let (client, mut sent) = client(vec![card_form("USD", 900), password_reply(), tmp, paid()]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        client
            .send_payment_form(form)
            .credentials(InputCredentials::saved("card-1", "hunter2"))
            .order_info_id("info-1")
            .shipping_option_id("ship-1")
            .tip_amount(50)
            .await
            .unwrap();
        sent.try_recv().unwrap();
        assert_eq!(sent.try_recv().unwrap(), functions::account::GetPassword {}.to_bytes());
        let tmp_request = sent.try_recv().unwrap();
        assert_eq!(tmp_request[..4], functions::account::GetTmpPassword::CONSTRUCTOR_ID.to_le_bytes());
        assert_eq!(tmp_request[tmp_request.len() - 4..], 60i32.to_le_bytes(), "the period is one minute");
        let saved = types::InputPaymentCredentialsSaved { id: "card-1".into(), tmp_password: vec![7; 32] };
        let saved = base::InputPaymentCredentials::InputPaymentCredentialsSaved(Box::new(saved));
        assert_eq!(sent.try_recv().unwrap(), card_payment(saved));
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn stars_form_with_credentials_is_refused_without_a_request() {
        let (client, mut sent) = client(vec![stars_form("XTR", &[900])]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        let result = client.send_payment_form(form).credentials(InputCredentials::new("{}")).await;
        assert!(matches!(result, Err(Error::InvalidArgument(_))), "{result:?}");
        sent.try_recv().unwrap();
        assert!(sent.try_recv().is_err(), "a Stars form must not be paid with a card");
    }

    #[tokio::test]
    async fn stars_form_with_order_info_shipping_or_tip_is_refused_without_a_request() {
        for which in 0..3 {
            let (client, mut sent) = client(vec![stars_form("XTR", &[900])]);
            let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
            let pay = client.send_payment_form(form);
            let pay = match which {
                0 => pay.order_info_id("o"),
                1 => pay.shipping_option_id("s"),
                _ => pay.tip_amount(5),
            };
            assert!(matches!(pay.await, Err(Error::InvalidArgument(_))), "{which}");
            sent.try_recv().unwrap();
            assert!(sent.try_recv().is_err(), "{which}: nothing may be paid");
        }
    }

    #[tokio::test]
    async fn ton_cap_pays_a_ton_form() {
        let ton = StarAmount::new(5).ton(true);
        let (first, mut sent) = client(vec![gift_form("TON", 5), paid()]);
        first.pay_form_up_to(slug(), ton).await.unwrap();
        let [get, pay] = payment(slug());
        assert_eq!(sent.try_recv().unwrap(), get);
        assert_eq!(sent.try_recv().unwrap(), pay);
        assert!(sent.try_recv().is_err());

        // A Toncoin cap never pays a Stars form, and a Stars cap never pays a Toncoin one.
        let (second, mut sent) = client(vec![gift_form("XTR", 5), gift_form("TON", 5)]);
        assert!(matches!(second.pay_form_up_to(slug(), ton).await, Err(Error::Unexpected(_))));
        assert!(matches!(second.pay_in_stars_up_to(slug(), 5).await, Err(Error::Unexpected(_))));
        sent.try_recv().unwrap();
        sent.try_recv().unwrap();
        assert!(sent.try_recv().is_err());
        let (third, mut sent) = client(vec![gift_form("TON", 6)]);
        assert!(matches!(third.pay_form_up_to(slug(), ton).await, Err(Error::InvalidArgument(_))));
        sent.try_recv().unwrap();
        assert!(sent.try_recv().is_err());
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
