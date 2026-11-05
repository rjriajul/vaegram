use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::ChatBoostUpdated;

impl Client {
    /// Registers a handler for boosts added to a chat that pass `filter`.
    ///
    /// Returns a `HandlerRef` to set the group, make the handler stop later groups, or get its id.
    pub fn on_chat_boost<F, Fut, T>(&self, filter: Filter<ChatBoostUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(ChatBoostUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::ChatBoost, |r, id| r.chat_boost.push(id, filter, callback(f)))
    }
}
