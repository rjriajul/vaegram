use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::DeletedBusinessMessages;

impl Client {
    /// Registers a handler for messages deleted through a business connection that pass `filter`.
    ///
    /// Returns a `HandlerRef` to set the group, make the handler stop later groups, or get its id.
    pub fn on_deleted_business_messages<F, Fut, T>(
        &self,
        filter: Filter<DeletedBusinessMessages>,
        f: F,
    ) -> HandlerRef<'_>
    where
        F: Fn(DeletedBusinessMessages) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::DeletedBusinessMessages, |r, id| {
            r.deleted_business_messages.push(id, filter, callback(f))
        })
    }
}
