use std::future::Future;
use std::sync::Arc;

use crate::Client;
use crate::errors::Result;
use crate::handlers::{BoxFuture, HandlerRef, RawCallback, UpdateKind};
use crate::types::RawUpdate;

impl Client {
    pub fn on_raw_update<F, Fut, T>(&self, callback: F) -> HandlerRef<'_>
    where
        F: Fn(Client, RawUpdate) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        let callback: RawCallback = Arc::new(move |c: Client, u: RawUpdate| {
            let future = callback(c, u);
            Box::pin(async move { future.await.map(drop) }) as BoxFuture
        });
        self.register(UpdateKind::Raw, |r, id| r.raw.push((id, callback)))
    }
}
