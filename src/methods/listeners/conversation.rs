use std::future::Future;

use crate::Client;
use crate::conversation::{Conversation, ConversationRef, ConversationUpdate};
use crate::errors::Result;

impl Client {
    /// Registers `f` as the conversation `name`, replacing the conversation of that name and its settings; the
    /// ones already running go on. [`enter_conversation`](Self::enter_conversation) starts it, with a [`Message`]
    /// or a [`CallbackQuery`] as `f`'s second argument, whichever `f` takes.
    ///
    /// `f` runs on a task of its own and waits for each next update with its [`Conversation`]: see
    /// [`conversation`](crate::conversation) for an example. An `Err` it returns goes to
    /// [`on_error`](Self::on_error) as a handler's would, with the update that started it; a wait that timed out,
    /// a client that stopped and [`Conversation::halt`] end it without a report. A panic is reported as
    /// `ClientEvent::HandlerPanicked`.
    ///
    /// [`Message`]: crate::types::Message
    /// [`CallbackQuery`]: crate::types::CallbackQuery
    pub fn conversation<T, F, Fut, R>(&self, name: &str, f: F) -> ConversationRef<'_>
    where
        T: ConversationUpdate,
        F: Fn(Conversation, T) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<R>> + Send + 'static,
    {
        self.0.conversations.define(name, f)
    }
}
