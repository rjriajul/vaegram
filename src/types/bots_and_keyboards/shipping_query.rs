use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::methods::AnswerShippingQuery;
use crate::raw::types;
use crate::types::{ShippingAddress, ShippingOption, User};

/// A buyer's shipping address, sent so the bot can offer shipping options and prices.
#[derive(Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ShippingQuery {
    /// Id of the query, used to answer it.
    pub id: i64,
    /// The user buying.
    pub from_user: User,
    /// The payload the bot set on the invoice.
    pub invoice_payload: String,
    /// The address to ship to.
    pub shipping_address: ShippingAddress,
}

impl ShippingQuery {
    pub(crate) fn parse(_client: &Client, raw: &types::UpdateBotShippingQuery, entities: &Entities) -> ShippingQuery {
        ShippingQuery {
            id: raw.query_id,
            from_user: User::parse(raw.user_id, entities),
            invoice_payload: String::from_utf8_lossy(&raw.payload).into_owned(),
            shipping_address: ShippingAddress::parse(&raw.shipping_address),
        }
    }

    /// Offers shipping options for the address. Await the returned request to send it.
    pub fn answer<'a>(
        &'a self,
        client: &'a Client,
        options: impl IntoIterator<Item = ShippingOption>,
    ) -> AnswerShippingQuery<'a> {
        client.answer_shipping_query(self.id).shipping_options(options)
    }

    /// Tells the user the order can't ship to this address, with `reason` as the message.
    pub async fn decline(&self, client: &Client, reason: impl Into<String>) -> Result<()> {
        client.answer_shipping_query(self.id).error(reason).await
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
