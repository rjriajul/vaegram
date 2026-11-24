use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::ChatMemberUpdated;

impl Client {
    /// Registers a handler for changes to a chat member's status that pass `filter`.
    ///
    /// Changes to this account's own membership go to [`Client::on_my_chat_member`] instead.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_chat_member_updated<F, Fut, T>(&self, filter: Filter<ChatMemberUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(ChatMemberUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::ChatMemberUpdated, |r, id| r.chat_member_updated.push(id, filter, callback(f)))
    }
}
