use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::TonWalletState;

impl Client {
    /// Registers a handler for changes to a TON wallet state that pass `filter`.
    ///
    /// Returns a `HandlerRef` to set the group, make the handler stop later groups, or get its id.
    pub fn on_ton_wallet_state<F, Fut, T>(&self, filter: Filter<TonWalletState>, f: F) -> HandlerRef<'_>
    where
        F: Fn(TonWalletState) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::TonWalletState, |r, id| r.ton_wallet_state.push(id, filter, callback(f)))
    }
}
