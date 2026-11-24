use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::Message;

impl Client {
    /// Registers a handler for edited messages received through a business connection that pass `filter`.
    ///
    /// What can be done with such a [`Message`] is described at
    /// [`on_business_message`](Client::on_business_message).
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_edited_business_message<F, Fut, T>(&self, filter: Filter<Message>, f: F) -> HandlerRef<'_>
    where
        F: Fn(Message) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::EditedBusinessMessage, |r, id| {
            r.edited_business_message.push(id, filter, callback(f))
        })
    }
}
