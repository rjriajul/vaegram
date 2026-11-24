use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::ReactionsUpdated;

impl Client {
    /// Registers a handler for changed reactions on a message, as user accounts receive them, that pass `filter`.
    /// Each update carries the message's whole new set of reactions. Bots get
    /// [`on_message_reaction`](Client::on_message_reaction) and
    /// [`on_message_reaction_count`](Client::on_message_reaction_count) instead.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_reactions<F, Fut, T>(&self, filter: Filter<ReactionsUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(ReactionsUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::Reactions, |r, id| r.reactions.push(id, filter, callback(f)))
    }
}
