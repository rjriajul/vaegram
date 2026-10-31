use super::Listen;
use crate::Client;
use crate::types::{Message, PeerRef};

impl Client {
    /// Waits for the next new message in `chat_id`: `listen::<Message>().chat(chat_id)`, with the same rules.
    pub fn wait_for_message(&self, chat_id: impl Into<PeerRef>) -> Listen<'_, Message> {
        self.listen().chat(chat_id)
    }
}
