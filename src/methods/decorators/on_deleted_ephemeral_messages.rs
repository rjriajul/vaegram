use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::DeletedEphemeralMessages;

impl Client {
    /// Registers a handler for deleted ephemeral messages that pass `filter`.
    ///
    /// Returns a `HandlerRef` to set the group, make the handler stop later groups, or get its id.
    pub fn on_deleted_ephemeral_messages<F, Fut, T>(
        &self,
        filter: Filter<DeletedEphemeralMessages>,
        f: F,
    ) -> HandlerRef<'_>
    where
        F: Fn(DeletedEphemeralMessages) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::DeletedEphemeralMessages, |r, id| {
            r.deleted_ephemeral_messages.push(id, filter, callback(f))
        })
    }
}
