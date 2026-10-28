use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::DeletedBusinessMessages;

impl Client {
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
