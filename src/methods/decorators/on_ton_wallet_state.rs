use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::TonWalletState;

impl Client {
    /// Registers a handler for changes to this account's TON wallet state that pass `filter`.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_ton_wallet_state<F, Fut, T>(&self, filter: Filter<TonWalletState>, f: F) -> HandlerRef<'_>
    where
        F: Fn(TonWalletState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::TonWalletState, |r, id| r.ton_wallet_state.push(id, filter, callback(f)))
    }
}
