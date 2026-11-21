use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::DraftUpdated;

impl Client {
    /// Registers a handler for drafts saved or cleared on any of this account's devices that pass `filter`.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_draft<F, Fut, T>(&self, filter: Filter<DraftUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(DraftUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::Draft, |r, id| r.draft.push(id, filter, callback(f)))
    }
}
