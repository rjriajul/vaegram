use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::ManagedBotUpdated;

impl Client {
    pub fn on_managed_bot<F, Fut, T>(&self, filter: Filter<ManagedBotUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(ManagedBotUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::ManagedBot, |r, id| r.managed_bot.push(id, filter, callback(f)))
    }
}
