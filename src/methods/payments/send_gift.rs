use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::methods::messages::caption_setters;
use crate::raw::{base, functions, types};
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct SendGift<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    gift_id: i64,
    text: Option<String>,
    parse_mode: Option<ParseMode>,
    pay_for_upgrade: bool,
    private: bool,
}

impl Client {
    pub fn send_gift(&self, chat_id: impl Into<PeerRef>, gift_id: i64) -> SendGift<'_> {
        SendGift {
            client: self,
            chat_id: chat_id.into(),
            gift_id,
            text: None,
            parse_mode: None,
            pay_for_upgrade: false,
            private: false,
        }
    }

    // Telegram accepts SendStarsForm only with the form_id GetPaymentForm returned for the same invoice.
    pub(crate) async fn pay_with_stars(
        &self,
        connection_id: Option<String>,
        invoice: base::InputInvoice,
    ) -> Result<()> {
        let form = functions::payments::GetPaymentForm { invoice, theme_params: None };
        let (form, request) = self.invoke_business(connection_id.as_deref(), form).await?;
        let form_id = match form {
            base::payments::PaymentForm::PaymentForm(f) => f.form_id,
            base::payments::PaymentForm::PaymentFormStars(f) => f.form_id,
            base::payments::PaymentForm::PaymentFormStarGift(f) => f.form_id,
        };
        let send = functions::payments::SendStarsForm { form_id, invoice: request.invoice };
        self.invoke_business(connection_id.as_deref(), send).await.map(drop)
    }

    pub(crate) async fn parse_text_with_entities(
        &self,
        text: Option<&str>,
        mode: Option<ParseMode>,
    ) -> Result<Option<base::TextWithEntities>> {
        let Some(text) = text else { return Ok(None) };
        let (text, entities) = self.parse_text(text, mode).await?;
        Ok(Some(base::TextWithEntities::TextWithEntities(Box::new(types::TextWithEntities { text, entities }))))
    }
}

impl SendGift<'_> {
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    pub fn pay_for_upgrade(mut self) -> Self {
        self.pay_for_upgrade = true;
        self
    }

    pub fn private(mut self) -> Self {
        self.private = true;
        self
    }

    async fn send(self) -> Result<()> {
        let client = self.client;
        let invoice = types::InputInvoiceStarGift {
            hide_name: self.private,
            include_upgrade: self.pay_for_upgrade,
            peer: client.resolve_peer(self.chat_id).await?,
            gift_id: self.gift_id,
            message: client.parse_text_with_entities(self.text.as_deref(), self.parse_mode).await?,
        };
        client.pay_with_stars(None, base::InputInvoice::InputInvoiceStarGift(Box::new(invoice))).await
    }
}

caption_setters!(SendGift);
into_future!(SendGift, ());
