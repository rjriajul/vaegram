use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{OrderInfo, PaymentForm, ShippingOption, ValidatedOrderInfo};

/// Asks the provider whether the order's contact and shipping details are good, and what shipping costs.
#[must_use = "a request does nothing until it is awaited"]
pub struct ValidateOrderInfo<'a> {
    client: &'a Client,
    invoice: base::InputInvoice,
    info: OrderInfo,
    save: bool,
}

impl Client {
    pub fn validate_order_info(&self, form: &PaymentForm, info: OrderInfo) -> ValidateOrderInfo<'_> {
        ValidateOrderInfo { client: self, invoice: form.invoice.clone(), info, save: false }
    }
}

impl ValidateOrderInfo<'_> {
    /// Remembers the details for the next order; [`get_saved_order_info`](Client::get_saved_order_info) reads them.
    pub fn save(mut self) -> Self {
        self.save = true;
        self
    }

    async fn send(self) -> Result<ValidatedOrderInfo> {
        let request = functions::payments::ValidateRequestedInfo {
            save: self.save,
            invoice: self.invoice,
            info: self.info.write(),
        };
        let base::payments::ValidatedRequestedInfo::ValidatedRequestedInfo(v) = self.client.invoke(&request).await?;
        let v = *v;
        Ok(ValidatedOrderInfo {
            id: v.id,
            shipping_options: v.shipping_options.unwrap_or_default().iter().map(ShippingOption::parse).collect(),
        })
    }
}

into_future!(ValidateOrderInfo, ValidatedOrderInfo);

#[cfg(test)]
mod tests {
    use super::super::send_payment_form::tests::{card_form, client};
    use crate::errors::Error;
    use crate::raw::{CREDENTIALS, Identifiable, Serializable, base, functions, types};
    use crate::types::{InvoiceRef, OrderInfo};

    fn info() -> OrderInfo {
        OrderInfo { name: Some("Ann".into()), phone_number: Some("+100".into()), email: None, shipping_address: None }
    }

    fn reply() -> Vec<u8> {
        let option = types::ShippingOption {
            id: "post".into(),
            title: "Post".into(),
            prices: vec![base::LabeledPrice::LabeledPrice(Box::new(types::LabeledPrice {
                label: "Stamp".into(),
                amount: 300,
            }))],
        };
        let reply = types::payments::ValidatedRequestedInfo {
            id: Some("info-1".into()),
            shipping_options: Some(vec![base::ShippingOption::ShippingOption(Box::new(option))]),
        };
        base::payments::ValidatedRequestedInfo::ValidatedRequestedInfo(Box::new(reply)).to_bytes()
    }

    #[tokio::test]
    async fn validate_sends_the_form_invoice_and_info() {
        let (client, mut sent) = client(vec![card_form("USD", 900), reply(), reply()]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        sent.try_recv().unwrap();
        let validated = client.validate_order_info(&form, info()).await.unwrap();
        let invoice = base::InputInvoice::InputInvoiceSlug(Box::new(types::InputInvoiceSlug { slug: "abc".into() }));
        let request =
            |save| functions::payments::ValidateRequestedInfo { save, invoice: invoice.clone(), info: info().write() };
        assert_eq!(sent.try_recv().unwrap(), request(false).to_bytes());
        assert_eq!(validated.id.as_deref(), Some("info-1"));
        assert_eq!(validated.shipping_options.len(), 1);
        let option = &validated.shipping_options[0];
        assert_eq!((option.id.as_str(), option.title.as_str(), option.prices[0].amount), ("post", "Post", 300));

        client.validate_order_info(&form, info()).save().await.unwrap();
        assert_eq!(sent.try_recv().unwrap(), request(true).to_bytes());
    }

    #[tokio::test]
    async fn a_rejected_info_returns_the_telegram_error() {
        let refused = Err(Error::Rpc(crate::errors::RpcError::new(400, "REQUESTED_INFO_INVALID")));
        let (client, _sent) = crate::Client::scripted(vec![Ok(card_form("USD", 900)), refused]);
        let form = client.get_payment_form(InvoiceRef::Link("abc".into())).await.unwrap();
        let result = client.validate_order_info(&form, info()).await;
        assert!(matches!(&result, Err(Error::Rpc(e)) if e.name == "REQUESTED_INFO_INVALID"), "{result:?}");
    }

    #[test]
    fn order_info_stays_out_of_recordings() {
        assert!(CREDENTIALS.contains(&<functions::payments::ValidateRequestedInfo as Identifiable>::CONSTRUCTOR_ID));
        assert!(CREDENTIALS.contains(&<types::payments::SavedInfo as Identifiable>::CONSTRUCTOR_ID));
        assert!(CREDENTIALS.contains(&<types::payments::PaymentForm as Identifiable>::CONSTRUCTOR_ID));
        assert!(CREDENTIALS.contains(&<types::PaymentRequestedInfo as Identifiable>::CONSTRUCTOR_ID));
    }
}
