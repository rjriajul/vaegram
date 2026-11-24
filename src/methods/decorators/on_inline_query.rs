use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::InlineQuery;

impl Client {
    /// Registers a handler for inline queries that pass `filter`.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_inline_query<F, Fut, T>(&self, filter: Filter<InlineQuery>, f: F) -> HandlerRef<'_>
    where
        F: Fn(InlineQuery) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::InlineQuery, |r, id| r.inline_query.push(id, filter, callback(f)))
    }
}
