use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::PreCheckoutQuery;

impl Client {
    pub fn on_pre_checkout_query<F, Fut, T>(&self, filter: Filter<PreCheckoutQuery>, f: F) -> HandlerRef<'_>
    where
        F: Fn(PreCheckoutQuery) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::PreCheckoutQuery, |r, id| r.pre_checkout_query.push(id, filter, callback(f)))
    }
}
