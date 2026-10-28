use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::MessageReactionUpdated;

impl Client {
    /// Bots receive these only for chats where they're admins.
    pub fn on_message_reaction<F, Fut, T>(&self, filter: Filter<MessageReactionUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(MessageReactionUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::MessageReaction, |r, id| r.message_reaction.push(id, filter, callback(f)))
    }
}
