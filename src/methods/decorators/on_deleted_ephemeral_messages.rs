use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::DeletedEphemeralMessages;

impl Client {
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
