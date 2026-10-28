use crate::Client;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::methods::messages::caption_setters;
use crate::raw::{base, types};
use crate::types::PeerRef;
use crate::utils::input_user;

#[must_use = "a request does nothing until it is awaited"]
pub struct GiftPremiumSubscription<'a> {
    client: &'a Client,
    user_id: PeerRef,
    month_count: i32,
    text: Option<String>,
    parse_mode: Option<ParseMode>,
}

impl Client {
    pub fn gift_premium_subscription(
        &self,
        user_id: impl Into<PeerRef>,
        month_count: i32,
    ) -> GiftPremiumSubscription<'_> {
        GiftPremiumSubscription { client: self, user_id: user_id.into(), month_count, text: None, parse_mode: None }
    }
}

impl GiftPremiumSubscription<'_> {
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    async fn send(self) -> Result<()> {
        let client = self.client;
        let peer = client.resolve(&self.user_id).await?.1;
        let invoice = types::InputInvoicePremiumGiftStars {
            user_id: input_user(peer).ok_or(Error::InvalidArgument("Premium can only be gifted to a user"))?,
            months: self.month_count,
            message: client.parse_text_with_entities(self.text.as_deref(), self.parse_mode).await?,
        };
        client.pay_with_stars(None, base::InputInvoice::InputInvoicePremiumGiftStars(Box::new(invoice))).await
    }
}

caption_setters!(GiftPremiumSubscription);
into_future!(GiftPremiumSubscription, ());
