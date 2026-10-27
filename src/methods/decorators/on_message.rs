use std::future::Future;
use std::sync::Arc;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{BoxFuture, Handler};
use crate::types::Message;

impl Client {
    pub fn on_message<F, Fut>(&self, filter: Filter, callback: F)
    where
        F: Fn(Message) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        let callback = Arc::new(move |m: Message| Box::pin(callback(m)) as BoxFuture);
        self.0.handlers.write().unwrap().push(Handler::Message(filter, callback));
    }
}
