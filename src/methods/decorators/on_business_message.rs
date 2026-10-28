use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::Message;

impl Client {
    pub fn on_business_message<F, Fut, T>(&self, filter: Filter<Message>, f: F) -> HandlerRef<'_>
    where
        F: Fn(Message) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::BusinessMessage, |r, id| r.business_message.push(id, filter, callback(f)))
    }
}
