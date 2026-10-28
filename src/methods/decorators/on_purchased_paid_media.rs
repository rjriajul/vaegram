use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::PurchasedPaidMedia;

impl Client {
    pub fn on_purchased_paid_media<F, Fut, T>(&self, filter: Filter<PurchasedPaidMedia>, f: F) -> HandlerRef<'_>
    where
        F: Fn(PurchasedPaidMedia) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::PurchasedPaidMedia, |r, id| r.purchased_paid_media.push(id, filter, callback(f)))
    }
}
