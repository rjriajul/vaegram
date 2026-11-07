use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::PendingJoinRequestsUpdated;

impl Client {
    /// Registers a handler for changes to the number of pending join requests in a chat this account
    /// administers that pass `filter`. Bots get each request on its own through
    /// [`on_chat_join_request`](Client::on_chat_join_request).
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_pending_join_requests<F, Fut, T>(
        &self,
        filter: Filter<PendingJoinRequestsUpdated>,
        f: F,
    ) -> HandlerRef<'_>
    where
        F: Fn(PendingJoinRequestsUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        let kind = UpdateKind::PendingJoinRequests;
        self.register(kind, |r, id| r.pending_join_requests.push(id, filter, callback(f)))
    }
}
