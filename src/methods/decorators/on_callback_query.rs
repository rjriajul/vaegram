use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::callback;
use crate::types::CallbackQuery;

impl Client {
    pub fn on_callback_query<F, Fut>(&self, filter: Filter<CallbackQuery>, f: F)
    where
        F: Fn(CallbackQuery) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.0.handlers.add(|r| r.callback_query.push(filter, callback(f)));
    }
}
