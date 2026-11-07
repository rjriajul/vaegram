use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::Message;

impl Client {
    /// Registers a handler for new messages received through a business connection that pass `filter`.
    ///
    /// Replies, edits, deletes, pins, read marks and chat actions made from such a [`Message`] go back through the
    /// same connection, to the data center that serves it.
    ///
    /// The message ids belong to the business account, so helpers that would look a message up in the bot's own
    /// chats refuse with `Error::InvalidArgument`: `forward`, `react`, `read_contents`, `get_media_group`,
    /// `copy_media_group`, the inline-result replies and `refetch`. `copy` sends the message as held. Listeners
    /// (`listen`, `ask`, `wait_for_message`) never see business messages, and a file reference in their media can't
    /// be renewed once it expires, so download what you need while handling the message.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_business_message<F, Fut, T>(&self, filter: Filter<Message>, f: F) -> HandlerRef<'_>
    where
        F: Fn(Message) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::BusinessMessage, |r, id| r.business_message.push(id, filter, callback(f)))
    }
}
