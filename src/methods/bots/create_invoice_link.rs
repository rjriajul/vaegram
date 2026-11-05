use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::InputInvoice;

/// Request built by [`Client::create_invoice_link`]. Awaiting it returns the link as a string.
#[must_use = "a request does nothing until it is awaited"]
pub struct CreateInvoiceLink<'a> {
    client: &'a Client,
    invoice: InputInvoice,
    business_connection_id: Option<String>,
}

impl Client {
    /// Creates a link that opens a payment form for the invoice. Awaiting the request returns the URL.
    pub fn create_invoice_link(&self, invoice: InputInvoice) -> CreateInvoiceLink<'_> {
        CreateInvoiceLink { client: self, invoice, business_connection_id: None }
    }
}

impl CreateInvoiceLink<'_> {
    /// Creates the link on behalf of the business account behind this connection.
    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.business_connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<String> {
        let request = functions::payments::ExportInvoice { invoice_media: self.invoice.write() };
        let (exported, _) = self.client.invoke_business(self.business_connection_id.as_deref(), request).await?;
        let base::payments::ExportedInvoice::ExportedInvoice(exported) = exported;
        Ok(exported.url)
    }
}

into_future!(CreateInvoiceLink, String);
