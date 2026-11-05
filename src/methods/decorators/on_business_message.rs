use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::Message;

impl Client {
    /// Registers a handler for new messages received through a business connection that pass `filter`.
    ///
    /// Returns a `HandlerRef` to set the group, make the handler stop later groups, or get its id.
    pub fn on_business_message<F, Fut, T>(&self, filter: Filter<Message>, f: F) -> HandlerRef<'_>
    where
        F: Fn(Message) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::BusinessMessage, |r, id| r.business_message.push(id, filter, callback(f)))
    }
}
