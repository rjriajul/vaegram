use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::PollUpdated;

impl Client {
    /// Registers a handler for changes to a poll, such as new results or closing, that pass `filter`.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_poll<F, Fut, T>(&self, filter: Filter<PollUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(PollUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::Poll, |r, id| r.poll.push(id, filter, callback(f)))
    }
}
