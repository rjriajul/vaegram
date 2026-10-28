use std::fmt;

use crate::Client;
use crate::dispatcher::Entities;
use crate::raw::types;
use crate::types::User;

#[derive(Clone)]
pub struct PurchasedPaidMedia {
    pub from_user: User,
    pub payload: String,
    client: Client,
}

impl PurchasedPaidMedia {
    pub(crate) fn parse(
        client: &Client,
        raw: &types::UpdateBotPurchasedPaidMedia,
        entities: &Entities,
    ) -> PurchasedPaidMedia {
        PurchasedPaidMedia {
            from_user: User::parse(raw.user_id, entities),
            payload: raw.payload.clone(),
            client: client.clone(),
        }
    }

    pub fn client(&self) -> &Client {
        &self.client
    }
}

impl fmt::Debug for PurchasedPaidMedia {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PurchasedPaidMedia")
            .field("from_user", &self.from_user)
            .field("payload", &self.payload)
            .finish_non_exhaustive()
    }
}
