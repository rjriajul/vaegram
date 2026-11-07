use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::PollUpdated;

impl Client {
    /// Registers a handler for poll changes, such as new results or closing, that pass `filter`.
    ///
    /// An update without the poll itself, which Telegram sends once the account has seen the poll, reaches the
    /// handler only when the message cache is on and kept the poll; see [`PollUpdated`].
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
