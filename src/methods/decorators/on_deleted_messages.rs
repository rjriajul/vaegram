use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::DeletedMessages;

impl Client {
    /// Registers a handler for deleted messages that pass `filter`.
    ///
    /// Returns a `HandlerRef` to set the group, make the handler stop later groups, or get its id.
    pub fn on_deleted_messages<F, Fut, T>(&self, filter: Filter<DeletedMessages>, f: F) -> HandlerRef<'_>
    where
        F: Fn(DeletedMessages) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::DeletedMessages, |r, id| r.deleted_messages.push(id, filter, callback(f)))
    }
}
