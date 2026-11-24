use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::handlers::{HandlerRef, UpdateKind, callback};

impl Client {
    /// Runs as `run` returns, after the running handlers finished and the update state was handed back, so it can
    /// still send. It also runs when a lost connection ended `run`, and then its requests fail. Only a `run` that
    /// got as far as `on_start` gets it. An `Err` goes to `on_error`. Registering again replaces it.
    pub fn on_stop<F, Fut, T>(&self, f: F) -> HandlerRef<'_>
    where
        F: Fn(Client) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::Stop, |r, id| r.hooks.set(UpdateKind::Stop, id, callback(f)))
    }
}
