use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::MessageGenerationStopped;

impl Client {
    pub fn on_message_generation_stopped<F, Fut, T>(
        &self,
        filter: Filter<MessageGenerationStopped>,
        f: F,
    ) -> HandlerRef<'_>
    where
        F: Fn(MessageGenerationStopped) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        let kind = UpdateKind::MessageGenerationStopped;
        self.register(kind, |r, id| r.message_generation_stopped.push(id, filter, callback(f)))
    }
}
