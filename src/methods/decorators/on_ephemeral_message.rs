use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::EphemeralMessage;

impl Client {
    pub fn on_ephemeral_message<F, Fut, T>(&self, filter: Filter<EphemeralMessage>, f: F) -> HandlerRef<'_>
    where
        F: Fn(EphemeralMessage) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::EphemeralMessage, |r, id| r.ephemeral_message.push(id, filter, callback(f)))
    }
}
