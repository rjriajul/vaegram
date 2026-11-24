use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::PreCheckoutQuery;

impl Client {
    /// Registers a handler for pre-checkout queries sent before a payment completes that pass `filter`.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_pre_checkout_query<F, Fut, T>(&self, filter: Filter<PreCheckoutQuery>, f: F) -> HandlerRef<'_>
    where
        F: Fn(PreCheckoutQuery) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::PreCheckoutQuery, |r, id| r.pre_checkout_query.push(id, filter, callback(f)))
    }
}
