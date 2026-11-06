use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{InvoiceRef, PaymentForm};

impl Client {
    /// Reads the price of an invoice.
    ///
    /// Nothing is paid until the form goes to [`Client::send_payment_form`].
    pub async fn get_payment_form(&self, invoice: InvoiceRef) -> Result<PaymentForm> {
        let invoice = invoice.write(self).await?;
        self.payment_form(invoice).await
    }

    pub(crate) async fn payment_form(&self, invoice: base::InputInvoice) -> Result<PaymentForm> {
        let request = functions::payments::GetPaymentForm { invoice, theme_params: None };
        let form = self.invoke(&request).await?;
        Ok(PaymentForm::parse(self, form, request.invoice))
    }
}

#[cfg(test)]
mod tests {
    use super::super::send_payment_form::tests::{
        card_form, client, gift_form, invoice, stars_form, stars_form_without_period,
    };
    use crate::enums::PaymentFormType;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::{InvoiceRef, LabeledPrice, OrderInfo};

    fn slug(slug: &str) -> base::InputInvoice {
        base::InputInvoice::InputInvoiceSlug(Box::new(types::InputInvoiceSlug { slug: slug.into() }))
    }

    #[tokio::test]
    async fn get_payment_form_sends_one_request_and_pays_nothing() {
        let (client, mut sent) = client(vec![stars_form("XTR", &[900]), stars_form("XTR", &[900])]);
        let form = client.get_payment_form(InvoiceRef::Link("t.me/$abc".into())).await.unwrap();
        let request = functions::payments::GetPaymentForm { invoice: slug("abc"), theme_params: None };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
        assert_eq!((form.id, form.total_amount, form.currency.as_str()), (7, 900, "XTR"));

        let message = InvoiceRef::Message { chat_id: 5.into(), message_id: 9 };
        client.get_payment_form(message).await.unwrap();
        let peer = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 5, access_hash: 6 }));
        let invoice = base::InputInvoice::InputInvoiceMessage(Box::new(types::InputInvoiceMessage { peer, msg_id: 9 }));
        let request = functions::payments::GetPaymentForm { invoice, theme_params: None };
        assert_eq!(sent.try_recv().unwrap(), request.to_bytes());
        assert!(sent.try_recv().is_err(), "reading a form must not pay it");
    }

    #[tokio::test]
    async fn a_link_without_a_slug_is_refused_without_a_request() {
        let (client, mut sent) = client(Vec::new());
        for link in ["", "t.me/$", "t.me/invoice/"] {
            let refused = client.get_payment_form(InvoiceRef::Link(link.into())).await;
            assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{link}");
        }
        assert!(sent.try_recv().is_err());
    }

    #[tokio::test]
    async fn maps_a_stars_form_with_its_bot() {
        let (client, _sent) = client(vec![stars_form("XTR", &[300, 600])]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        assert_eq!(form.bot.as_ref().map(|b| (b.id, b.first_name.as_str())), Some((8, "Shop")));
        assert_eq!((form.title.as_deref(), form.description.as_deref()), (Some("Pro"), Some("A month of Pro")));
        assert_eq!(form.prices, [LabeledPrice::new("Item", 300), LabeledPrice::new("Item", 600)]);
        assert_eq!((form.total_amount, form.subscription_period, form.is_test), (900, Some(2592000), true));
        assert_eq!(form.invoice, slug("abc"));
    }

    #[tokio::test]
    async fn a_star_gift_form_has_no_bot_or_title() {
        let (client, _sent) = client(vec![gift_form("XTR", 50)]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        assert_eq!((form.bot, form.title, form.description), (None, None, None));
        assert_eq!((form.id, form.total_amount), (7, 50));
    }

    #[tokio::test]
    async fn a_card_form_keeps_its_currency() {
        let form = types::payments::PaymentForm {
            can_save_credentials: false,
            password_missing: false,
            form_id: 3,
            bot_id: 8,
            title: "Pro".into(),
            description: "A month of Pro".into(),
            photo: None,
            invoice: invoice("USD", &[100]),
            provider_id: 1,
            url: String::new(),
            native_provider: None,
            native_params: None,
            additional_methods: None,
            saved_info: None,
            saved_credentials: None,
            users: Vec::new(),
        };
        let form = base::payments::PaymentForm::PaymentForm(Box::new(form)).to_bytes();
        let (client, _sent) = client(vec![form]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        assert_eq!((form.id, form.currency.as_str(), form.total_amount), (3, "USD", 100));
        assert_eq!(form.bot.map(|b| b.id), Some(8));
    }

    #[tokio::test]
    async fn card_form_reads_provider_url_options_and_saved_data() {
        let (client, _sent) = client(vec![card_form("USD", 900)]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        assert_eq!(form.kind, PaymentFormType::Regular);
        assert_eq!(form.payment_provider.as_ref().map(|p| (p.id, p.first_name.as_str())), Some((9, "Provider")));
        assert_eq!(form.url.as_deref(), Some("https://pay.example.com/form"));
        assert_eq!(
            (form.native_provider.as_deref(), form.native_params.as_deref()),
            (Some("stripe"), Some("{\"k\":1}"))
        );
        assert_eq!(form.photo_url.as_deref(), Some("https://example.com/p.jpg"));
        let options: Vec<_> =
            form.additional_payment_options.iter().map(|o| (o.title.as_str(), o.url.as_str())).collect();
        assert_eq!(options, [("Wallet", "https://wallet.example.com")]);
        let cards: Vec<_> = form.saved_credentials.iter().map(|c| (c.id.as_str(), c.title.as_str())).collect();
        assert_eq!(cards, [("card-1", "Visa *1111")]);
        let info = OrderInfo::new().name("Ann");
        assert_eq!(form.saved_order_info, Some(info));
        assert!(form.can_save_credentials && form.need_password);
    }

    #[tokio::test]
    async fn debug_hides_saved_order_info_and_card_titles() {
        let (client, _sent) = client(vec![card_form("USD", 900)]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        let shown = format!("{form:?}");
        assert!(shown.contains("saved_order_info: <redacted>"), "{shown}");
        for private in ["Ann", "Visa *1111", "card-1"] {
            assert!(!shown.contains(private), "{private} leaked: {shown}");
        }
    }

    #[tokio::test]
    async fn invoice_flags_and_tips_are_flat_on_the_form() {
        let flagged = types::Invoice {
            test: false,
            name_requested: true,
            phone_requested: true,
            email_requested: true,
            shipping_address_requested: true,
            flexible: true,
            phone_to_provider: true,
            email_to_provider: true,
            recurring: true,
            currency: "USD".into(),
            prices: Vec::new(),
            max_tip_amount: Some(500),
            suggested_tip_amounts: Some(vec![100, 200]),
            terms_url: Some("https://example.com/terms".into()),
            subscription_period: None,
        };
        let form =
            types::payments::PaymentFormStarGift { form_id: 7, invoice: base::Invoice::Invoice(Box::new(flagged)) };
        let form = base::payments::PaymentForm::PaymentFormStarGift(Box::new(form)).to_bytes();
        let (client, _sent) = client(vec![form]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        assert!(form.is_name_requested && form.is_phone_requested && form.is_email_requested);
        assert!(form.is_shipping_address_requested && form.is_flexible && form.is_recurring);
        assert!(form.is_phone_to_provider && form.is_email_to_provider);
        assert_eq!((form.max_tip_amount, form.suggested_tip_amounts.as_slice()), (Some(500), &[100, 200][..]));
        assert_eq!(form.terms_url.as_deref(), Some("https://example.com/terms"));
        assert!(form.url.is_none() && form.payment_provider.is_none() && form.saved_credentials.is_empty());
    }

    #[tokio::test]
    async fn form_kinds_map_regular_stars_subscription_and_gift() {
        let (client, _sent) = client(vec![
            card_form("USD", 1),
            stars_form("XTR", &[1]),
            gift_form("XTR", 1),
            stars_form_without_period(),
        ]);
        let mut kinds = Vec::new();
        for _ in 0..4 {
            kinds.push(client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap().kind);
        }
        assert_eq!(
            kinds,
            [
                PaymentFormType::Regular,
                PaymentFormType::StarSubscription,
                PaymentFormType::StarGift,
                PaymentFormType::Stars
            ]
        );
    }

    #[tokio::test]
    async fn overflowing_prices_saturate() {
        let (client, _sent) = client(vec![stars_form("XTR", &[i64::MAX, 1])]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        assert_eq!(form.total_amount, i64::MAX);
    }

    #[tokio::test]
    async fn rpc_errors_come_back_unchanged() {
        let (client, _sent) =
            crate::Client::scripted(vec![Err(Error::Rpc(crate::errors::RpcError::parse(400, "SLUG_INVALID")))]);
        let result = client.get_payment_form(InvoiceRef::Link("abc".into())).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "SLUG_INVALID"), "{result:?}");
    }
}
