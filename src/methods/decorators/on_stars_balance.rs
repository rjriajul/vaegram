use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::StarsBalanceUpdated;

impl Client {
    /// Registers a handler for changes to this account's Telegram Stars balance that pass `filter`.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_stars_balance<F, Fut, T>(&self, filter: Filter<StarsBalanceUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(StarsBalanceUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::StarsBalance, |r, id| r.stars_balance.push(id, filter, callback(f)))
    }
}
