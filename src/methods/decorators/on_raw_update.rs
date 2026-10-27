use std::future::Future;
use std::sync::Arc;

use crate::Client;
use crate::errors::Result;
use crate::handlers::{BoxFuture, Handler};
use crate::raw::base;

impl Client {
    pub fn on_raw_update<F, Fut>(&self, callback: F)
    where
        F: Fn(Client, base::Update) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        let callback = Arc::new(move |c: Client, u: base::Update| Box::pin(callback(c, u)) as BoxFuture);
        self.0.handlers.write().unwrap().push(Handler::RawUpdate(callback));
    }
}
