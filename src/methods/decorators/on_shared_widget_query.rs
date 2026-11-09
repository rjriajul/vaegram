use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::SharedWidgetQuery;

impl Client {
    /// Registers a handler for shared widget queries that pass `filter`.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_shared_widget_query<F, Fut, T>(&self, filter: Filter<SharedWidgetQuery>, f: F) -> HandlerRef<'_>
    where
        F: Fn(SharedWidgetQuery) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        let kind = UpdateKind::SharedWidgetQuery;
        self.register(kind, |r, id| r.shared_widget_query.push(id, filter, callback(f)))
    }
}
