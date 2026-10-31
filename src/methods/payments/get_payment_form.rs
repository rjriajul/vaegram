use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{InvoiceRef, PaymentForm};

impl Client {
    /// Reads the price of an invoice; nothing is paid until the form goes to [`Client::send_payment_form`].
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
    use super::super::send_payment_form::tests::{client, gift_form, invoice, stars_form};
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::{InvoiceRef, LabeledPrice};

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
    async fn overflowing_prices_saturate() {
        let (client, _sent) = client(vec![stars_form("XTR", &[i64::MAX, 1])]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        assert_eq!(form.total_amount, i64::MAX);
    }

    #[tokio::test]
    async fn rpc_errors_come_back_unchanged() {
        let (client, _sent) =
            crate::Client::scripted(vec![Err(Error::Rpc(crate::errors::RpcError::new(400, "SLUG_INVALID")))]);
        let result = client.get_payment_form(InvoiceRef::Link("abc".into())).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "SLUG_INVALID"), "{result:?}");
    }
}
