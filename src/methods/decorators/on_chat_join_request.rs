use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::ChatJoinRequest;

impl Client {
    /// Registers a handler for requests to join a chat that pass `filter`.
    ///
    /// Returns a `HandlerRef` to set the group, make the handler stop later groups, or get its id.
    pub fn on_chat_join_request<F, Fut, T>(&self, filter: Filter<ChatJoinRequest>, f: F) -> HandlerRef<'_>
    where
        F: Fn(ChatJoinRequest) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::ChatJoinRequest, |r, id| r.chat_join_request.push(id, filter, callback(f)))
    }
}
