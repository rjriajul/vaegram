use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::GuestMessage;

impl Client {
    /// Messages in chats the bot isn't in, sent through it by a user; answer with `GuestMessage::answer`.
    pub fn on_guest_message<F, Fut, T>(&self, filter: Filter<GuestMessage>, f: F) -> HandlerRef<'_>
    where
        F: Fn(GuestMessage) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::GuestMessage, |r, id| r.guest_message.push(id, filter, callback(f)))
    }
}
