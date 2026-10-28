use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::ChatBoostRemoved;

impl Client {
    pub fn on_removed_chat_boost<F, Fut, T>(&self, filter: Filter<ChatBoostRemoved>, f: F) -> HandlerRef<'_>
    where
        F: Fn(ChatBoostRemoved) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::RemovedChatBoost, |r, id| r.removed_chat_boost.push(id, filter, callback(f)))
    }
}
