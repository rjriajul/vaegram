use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct TransferGift<'a> {
    client: &'a Client,
    owned_gift_id: String,
    new_owner_chat_id: PeerRef,
    pay_for_transfer: bool,
    connection_id: Option<String>,
}

impl Client {
    pub fn transfer_gift(
        &self,
        owned_gift_id: impl Into<String>,
        new_owner_chat_id: impl Into<PeerRef>,
    ) -> TransferGift<'_> {
        TransferGift {
            client: self,
            owned_gift_id: owned_gift_id.into(),
            new_owner_chat_id: new_owner_chat_id.into(),
            pay_for_transfer: false,
            connection_id: None,
        }
    }
}

impl TransferGift<'_> {
    /// Pays the transfer fee in Stars; needed when the gift's transfer_star_count is set.
    pub fn pay_for_transfer(mut self) -> Self {
        self.pay_for_transfer = true;
        self
    }

    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<()> {
        let client = self.client;
        let stargift = client.input_saved_gift(&self.owned_gift_id).await?;
        let to_id = client.resolve_peer(self.new_owner_chat_id).await?;
        if self.pay_for_transfer {
            let invoice = types::InputInvoiceStarGiftTransfer { stargift, to_id };
            let invoice = base::InputInvoice::InputInvoiceStarGiftTransfer(Box::new(invoice));
            return client.pay_with_stars(self.connection_id, invoice).await;
        }
        let request = functions::payments::TransferStarGift { stargift, to_id };
        client.invoke_business(self.connection_id.as_deref(), request).await.map(drop)
    }
}

into_future!(TransferGift, ());
