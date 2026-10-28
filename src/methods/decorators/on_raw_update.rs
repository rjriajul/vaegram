use std::future::Future;
use std::sync::Arc;

use crate::Client;
use crate::errors::Result;
use crate::handlers::{BoxFuture, HandlerRef, UpdateKind};
use crate::raw::base;

impl Client {
    pub fn on_raw_update<F, Fut, T>(&self, callback: F) -> HandlerRef<'_>
    where
        F: Fn(Client, base::Update) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        let callback = Arc::new(move |c: Client, u: base::Update| {
            let future = callback(c, u);
            Box::pin(async move { future.await.map(drop) }) as BoxFuture
        });
        self.register(UpdateKind::Raw, |r, _| r.raw.push(callback))
    }
}
