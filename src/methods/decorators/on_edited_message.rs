use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::callback;
use crate::types::Message;

impl Client {
    pub fn on_edited_message<F, Fut>(&self, filter: Filter<Message>, f: F)
    where
        F: Fn(Message) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.0.handlers.add(|r| r.edited_message.push(filter, callback(f)));
    }
}
