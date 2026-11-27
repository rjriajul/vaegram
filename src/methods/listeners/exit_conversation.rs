use crate::Client;
use crate::conversation::ConversationUpdate;

impl Client {
    /// Ends the conversation `name` running for `update`'s chat and user, as the scope it started under keys them
    /// (every one of them when it is parallel), and returns how many it ended; `0` when none runs or no
    /// conversation is registered as `name`. Under a topic scope a message names its topic; a press doesn't, so a
    /// press ends the user's (or chat's) conversation `name` in every topic. An update through a business
    /// connection ends none.
    ///
    /// The conversation stops at once, wherever it is, without a report; the updates it took and hadn't read go on to
    /// the handlers, never to another conversation. Register the command that calls this with the conversation's
    /// [`ignore`](crate::conversation::ConversationRef::ignore) or
    /// [`ignore_commands`](crate::conversation::ConversationRef::ignore_commands), or the conversation takes the
    /// command first. `stop_listening` never ends a conversation.
    pub fn exit_conversation<T: ConversationUpdate>(&self, name: &str, update: &T) -> usize {
        self.0.conversations.exit(self, name, update)
    }
}
