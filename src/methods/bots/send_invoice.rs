use crate::Client;
use crate::errors::Result;
use crate::methods::messages::{SendOptions, send_builder};
use crate::types::{InputInvoice, Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendInvoice<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    invoice: InputInvoice,
    options: SendOptions,
}

impl Client {
    /// A custom inline keyboard must start with a Pay button.
    pub fn send_invoice(&self, chat_id: impl Into<PeerRef>, invoice: InputInvoice) -> SendInvoice<'_> {
        SendInvoice { client: self, chat_id: chat_id.into(), invoice, options: SendOptions::default() }
    }
}

impl SendInvoice<'_> {
    async fn send(self) -> Result<Message> {
        self.client.send_media(self.chat_id, self.invoice.write(), "", None, self.options).await
    }
}

send_builder!(SendInvoice);
