use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::AnswerShippingQuery;
use crate::raw::types;
use crate::types::{ShippingAddress, ShippingOption, User};

#[derive(Clone)]
pub struct ShippingQuery {
    pub id: i64,
    pub from_user: User,
    pub invoice_payload: String,
    pub shipping_address: ShippingAddress,
    client: Client,
}

impl ShippingQuery {
    pub(crate) fn parse(client: &Client, raw: &types::UpdateBotShippingQuery, entities: &Entities) -> ShippingQuery {
        ShippingQuery {
            id: raw.query_id,
            from_user: User::parse(raw.user_id, entities),
            invoice_payload: String::from_utf8_lossy(&raw.payload).into_owned(),
            shipping_address: ShippingAddress::parse(&raw.shipping_address),
            client: client.clone(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub fn answer(&self, options: impl IntoIterator<Item = ShippingOption>) -> AnswerShippingQuery<'_> {
        self.client.answer_shipping_query(self.id).shipping_options(options)
    }

    pub async fn decline(&self, reason: impl Into<String>) -> Result<()> {
        self.client.answer_shipping_query(self.id).error(reason).await.map(drop)
    }
}

impl fmt::Debug for ShippingQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ShippingQuery")
            .field("id", &self.id)
            .field("from_user", &self.from_user)
            .field("invoice_payload", &self.invoice_payload)
            .field("shipping_address", &self.shipping_address)
            .finish_non_exhaustive()
    }
}
