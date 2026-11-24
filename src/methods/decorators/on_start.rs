use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::handlers::{HandlerRef, UpdateKind, callback};

impl Client {
    /// Runs when `run` starts, before the first update; `run` waits for it. An `Err` goes to `on_error` and `run`
    /// carries on. Registering again replaces it; `group` and `stop` don't apply.
    pub fn on_start<F, Fut, T>(&self, f: F) -> HandlerRef<'_>
    where
        F: Fn(Client) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::Start, |r, id| r.hooks.set(UpdateKind::Start, id, callback(f)))
    }
}
