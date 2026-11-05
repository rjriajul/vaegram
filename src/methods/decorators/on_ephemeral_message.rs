use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::EphemeralMessage;

impl Client {
    /// Registers a handler for new ephemeral messages that pass `filter`.
    ///
    /// Returns a `HandlerRef` to set the group, make the handler stop later groups, or get its id.
    pub fn on_ephemeral_message<F, Fut, T>(&self, filter: Filter<EphemeralMessage>, f: F) -> HandlerRef<'_>
    where
        F: Fn(EphemeralMessage) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::EphemeralMessage, |r, id| r.ephemeral_message.push(id, filter, callback(f)))
    }
}
