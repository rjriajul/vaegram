use std::future::Future;
use std::sync::Arc;

use crate::Client;
use crate::errors::Result;
use crate::handlers::{BoxFuture, HandlerRef, RawCallback, UpdateKind};
use crate::types::RawUpdate;

impl Client {
    /// Registers a handler that receives every update as a [`RawUpdate`], with no filter, except the ones that come
    /// back inside the answers to this client's own requests (see [`handlers`](crate::handlers)).
    ///
    /// Raw handlers always run and ignore the handler group. Returns a [`HandlerRef`] that gives the id for removal.
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
