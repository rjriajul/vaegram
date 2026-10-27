use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::callback;
use crate::types::InlineQuery;

impl Client {
    pub fn on_inline_query<F, Fut>(&self, filter: Filter<InlineQuery>, f: F)
    where
        F: Fn(InlineQuery) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.0.handlers.add(|r| r.inline_query.push(filter, callback(f)));
    }
}
