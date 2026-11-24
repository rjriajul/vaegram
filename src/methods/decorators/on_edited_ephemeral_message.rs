use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::EphemeralMessage;

impl Client {
    /// Registers a handler for edited ephemeral messages that pass `filter`.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_edited_ephemeral_message<F, Fut, T>(&self, filter: Filter<EphemeralMessage>, f: F) -> HandlerRef<'_>
    where
        F: Fn(EphemeralMessage) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::EditedEphemeralMessage, |r, id| {
            r.edited_ephemeral_message.push(id, filter, callback(f))
        })
    }
}
