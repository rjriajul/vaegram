use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::ShippingQuery;

impl Client {
    /// Registers a handler for shipping queries for invoices with a flexible price that pass `filter`.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_shipping_query<F, Fut, T>(&self, filter: Filter<ShippingQuery>, f: F) -> HandlerRef<'_>
    where
        F: Fn(ShippingQuery) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::ShippingQuery, |r, id| r.shipping_query.push(id, filter, callback(f)))
    }
}
