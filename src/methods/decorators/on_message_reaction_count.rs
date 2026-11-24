use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::MessageReactionCountUpdated;

impl Client {
    /// Bots receive these only for anonymous reactions in chats where they're admins.
    pub fn on_message_reaction_count<F, Fut, T>(
        &self,
        filter: Filter<MessageReactionCountUpdated>,
        f: F,
    ) -> HandlerRef<'_>
    where
        F: Fn(MessageReactionCountUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        let kind = UpdateKind::MessageReactionCount;
        self.register(kind, |r, id| r.message_reaction_count.push(id, filter, callback(f)))
    }
}
