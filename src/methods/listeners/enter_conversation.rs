use crate::Client;
use crate::conversation::ConversationUpdate;
use crate::errors::Result;

impl Client {
    /// Starts the conversation `name` with `update` (usually the one a handler got) and returns at once: the
    /// conversation runs on a task of its own, so the handler finishes and frees its slot.
    ///
    /// Its listeners start before this returns, so nothing the user sends from here on is missed. Fails with a
    /// [`ConversationError`](crate::conversation::ConversationError) inside `Error::Other`: `NotRegistered`,
    /// `WrongUpdate` when the conversation starts with the other update kind, `NoKey` when `update` lacks the chat
    /// or user its scope keys by, is a press under a topic scope or came through a business connection, and `Busy`
    /// when a conversation already runs for the same key and they aren't
    /// both parallel. Fails with `Error::ListenerStopped` between runs and `Error::ListenerLimitReached` past
    /// `max_listeners`.
    pub fn enter_conversation<T: ConversationUpdate>(&self, name: &str, update: T) -> Result<()> {
        self.0.conversations.enter(self, name, update)
    }
}
