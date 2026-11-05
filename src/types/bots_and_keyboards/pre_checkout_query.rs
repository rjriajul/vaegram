use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::types;
use crate::types::{OrderInfo, User};

/// A buyer's final confirmation before paying an invoice. Answer it with [`approve`](Self::approve) or
/// [`decline`](Self::decline).
#[derive(Clone)]
pub struct PreCheckoutQuery {
    /// Id of the query, used to answer it.
    pub id: i64,
    /// The user paying.
    pub from_user: User,
    /// Currency code of the invoice.
    pub currency: String,
    /// Total price in the smallest unit of `currency`.
    pub total_amount: i64,
    /// The payload the bot set on the invoice.
    pub invoice_payload: String,
    /// Id of the [`ShippingOption`](crate::types::ShippingOption) the user chose. `None` if there was none to
    /// choose.
    pub shipping_option_id: Option<String>,
    /// Details the user gave at checkout. `None` if the invoice asked for none.
    pub order_info: Option<OrderInfo>,
    client: Client,
}

impl PreCheckoutQuery {
    pub(crate) fn parse(
        client: &Client,
        raw: &types::UpdateBotPrecheckoutQuery,
        entities: &Entities,
    ) -> PreCheckoutQuery {
        PreCheckoutQuery {
            id: raw.query_id,
            from_user: User::parse(raw.user_id, entities),
            currency: raw.currency.clone(),
            total_amount: raw.total_amount,
            invoice_payload: String::from_utf8_lossy(&raw.payload).into_owned(),
            shipping_option_id: raw.shipping_option_id.clone(),
            order_info: raw.info.as_ref().map(OrderInfo::parse),
            client: client.clone(),
        }
    }

    /// The client that received the query.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Confirms the payment can go ahead.
    ///
    /// Telegram cancels the payment unless the query is answered within 10 seconds.
    pub async fn approve(&self) -> Result<()> {
        self.client.answer_pre_checkout_query(self.id).await.map(drop)
    }

    /// Rejects the payment and shows `reason` to the user.
    pub async fn decline(&self, reason: impl Into<String>) -> Result<()> {
        self.client.answer_pre_checkout_query(self.id).error(reason).await.map(drop)
    }
}

impl fmt::Debug for PreCheckoutQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PreCheckoutQuery")
            .field("id", &self.id)
            .field("from_user", &self.from_user)
            .field("currency", &self.currency)
            .field("total_amount", &self.total_amount)
            .field("invoice_payload", &self.invoice_payload)
            .field("shipping_option_id", &self.shipping_option_id)
            .field("order_info", &self.order_info)
            .finish_non_exhaustive()
    }
}
