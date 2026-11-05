use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::ChosenInlineResult;

impl Client {
    /// Registers a handler for inline results that a user picked that pass `filter`.
    ///
    /// Returns a `HandlerRef` to set the group, make the handler stop later groups, or get its id.
    pub fn on_chosen_inline_result<F, Fut, T>(&self, filter: Filter<ChosenInlineResult>, f: F) -> HandlerRef<'_>
    where
        F: Fn(ChosenInlineResult) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::ChosenInlineResult, |r, id| r.chosen_inline_result.push(id, filter, callback(f)))
    }
}
