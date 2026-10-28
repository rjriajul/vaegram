use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions, types};

#[must_use = "a request does nothing until it is awaited"]
pub struct UpgradeGift<'a> {
    client: &'a Client,
    owned_gift_id: String,
    keep_original_details: bool,
    pay_for_upgrade: bool,
    connection_id: Option<String>,
}

impl Client {
    pub fn upgrade_gift(&self, owned_gift_id: impl Into<String>) -> UpgradeGift<'_> {
        UpgradeGift {
            client: self,
            owned_gift_id: owned_gift_id.into(),
            keep_original_details: false,
            pay_for_upgrade: false,
            connection_id: None,
        }
    }
}

impl UpgradeGift<'_> {
    pub fn keep_original_details(mut self) -> Self {
        self.keep_original_details = true;
        self
    }

    /// Pays the upgrade price in Stars; needed unless the sender prepaid the upgrade.
    pub fn pay_for_upgrade(mut self) -> Self {
        self.pay_for_upgrade = true;
        self
    }

    pub fn business_connection_id(mut self, id: impl Into<String>) -> Self {
        self.connection_id = Some(id.into());
        self
    }

    async fn send(self) -> Result<()> {
        let client = self.client;
        let stargift = client.input_saved_gift(&self.owned_gift_id).await?;
        let keep_original_details = self.keep_original_details;
        if self.pay_for_upgrade {
            let invoice = types::InputInvoiceStarGiftUpgrade { keep_original_details, stargift };
            let invoice = base::InputInvoice::InputInvoiceStarGiftUpgrade(Box::new(invoice));
            return client.pay_with_stars(self.connection_id, invoice).await;
        }
        let request = functions::payments::UpgradeStarGift { keep_original_details, stargift };
        client.invoke_business(self.connection_id.as_deref(), request).await.map(drop)
    }
}

into_future!(UpgradeGift, ());
