use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::types;
use crate::types::{OrderInfo, User};

#[derive(Clone)]
pub struct PreCheckoutQuery {
    pub id: i64,
    pub from_user: User,
    pub currency: String,
    pub total_amount: i64,
    pub invoice_payload: String,
    pub shipping_option_id: Option<String>,
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

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub async fn approve(&self) -> Result<()> {
        self.client.answer_pre_checkout_query(self.id).await.map(drop)
    }

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
