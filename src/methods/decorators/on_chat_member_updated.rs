use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::ChatMemberUpdated;

impl Client {
    /// Registers a handler for changes to a chat member's status that pass `filter`.
    ///
    /// Returns a `HandlerRef` to set the group, make the handler stop later groups, or get its id.
    pub fn on_chat_member_updated<F, Fut, T>(&self, filter: Filter<ChatMemberUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(ChatMemberUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::ChatMemberUpdated, |r, id| r.chat_member_updated.push(id, filter, callback(f)))
    }
}
