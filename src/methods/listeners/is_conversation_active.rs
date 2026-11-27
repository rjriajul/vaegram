use crate::Client;
use crate::conversation::ConversationUpdate;

impl Client {
    /// Whether the conversation `name` runs for `update`'s chat and user, as the scope it started under keys them.
    /// Under a topic scope a press, which doesn't say its topic, finds the conversation in any topic.
    pub fn is_conversation_active<T: ConversationUpdate>(&self, name: &str, update: &T) -> bool {
        self.0.conversations.is_active(name, update)
    }
}
