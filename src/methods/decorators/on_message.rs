use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{Handler, callback};
use crate::types::Message;

impl Client {
    pub fn on_message<F, Fut>(&self, filter: Filter<Message>, f: F)
    where
        F: Fn(Message) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.0.handlers.write().unwrap().push(Handler::Message(filter, callback(f)));
    }
}
