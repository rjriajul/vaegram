use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::WidgetDataUpdated;

impl Client {
    /// Registers a handler for widget data changes that pass `filter`.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_widget_data_updated<F, Fut, T>(&self, filter: Filter<WidgetDataUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(WidgetDataUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        let kind = UpdateKind::WidgetDataUpdated;
        self.register(kind, |r, id| r.widget_data_updated.push(id, filter, callback(f)))
    }
}
