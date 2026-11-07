use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::PinnedMessagesUpdated;

impl Client {
    /// Registers a handler for messages pinned or unpinned in a chat that pass `filter`. A pin can also post a
    /// service message, which reaches [`on_message`](Client::on_message) on its own.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_pinned_messages<F, Fut, T>(&self, filter: Filter<PinnedMessagesUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(PinnedMessagesUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::PinnedMessages, |r, id| r.pinned_messages.push(id, filter, callback(f)))
    }
}
