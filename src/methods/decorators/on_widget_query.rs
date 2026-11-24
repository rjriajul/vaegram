use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::WidgetQuery;

impl Client {
    /// Registers a handler for widget queries that pass `filter`.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_widget_query<F, Fut, T>(&self, filter: Filter<WidgetQuery>, f: F) -> HandlerRef<'_>
    where
        F: Fn(WidgetQuery) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        let kind = UpdateKind::WidgetQuery;
        self.register(kind, |r, id| r.widget_query.push(id, filter, callback(f)))
    }
}
