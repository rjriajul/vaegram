use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::CallbackQuery;

impl Client {
    pub fn on_callback_query<F, Fut, T>(&self, filter: Filter<CallbackQuery>, f: F) -> HandlerRef<'_>
    where
        F: Fn(CallbackQuery) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::CallbackQuery, |r, id| r.callback_query.push(id, filter, callback(f)))
    }
}
