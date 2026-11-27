use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::Update;

impl Client {
    /// Registers a handler for the updates no other handler took, that pass `filter`: no listener claimed them and
    /// no typed `on_*` handler's filter matched. [`on_raw_update`](Client::on_raw_update) handlers see every update
    /// and don't count. A message a listener claims and later gives back counts as taken. An update that reaches an
    /// included [`Router`](crate::handlers::Router) is decided once the router is done with it, in the update's own
    /// task: it counts as taken when a handler ran or an outer middleware kept it.
    ///
    /// The [`Update`] holds the typed event, or [`Update::Other`] when vaegram has none for it yet. Updates move out
    /// of `Other` as typed events are added, so match the variants you need and keep a `_` arm.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_unhandled_update<F, Fut, T>(&self, filter: Filter<Update>, f: F) -> HandlerRef<'_>
    where
        F: Fn(Update) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::Unhandled, |r, id| r.unhandled.push(id, filter, callback(f)))
    }
}
