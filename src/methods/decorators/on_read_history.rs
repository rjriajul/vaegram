use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::ReadHistoryUpdated;

impl Client {
    /// Registers a handler for read receipts that pass `filter`: this account reading incoming messages (also on
    /// its other devices), or the other side reading messages it sent. Covers chats, channels, forum topics and
    /// comment threads.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_read_history<F, Fut, T>(&self, filter: Filter<ReadHistoryUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(ReadHistoryUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::ReadHistory, |r, id| r.read_history.push(id, filter, callback(f)))
    }
}
