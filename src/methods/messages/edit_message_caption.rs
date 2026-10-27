use crate::Client;
use crate::methods::EditMessageText;
use crate::types::{MessageText, PeerRef, Text};

impl Client {
    /// An empty caption removes it.
    pub fn edit_message_caption(
        &self,
        chat_id: impl Into<PeerRef>,
        message_id: i32,
        caption: impl Into<Text>,
    ) -> EditMessageText<'_> {
        self.edit_message_text(chat_id, message_id, MessageText::Text(caption.into()))
    }
}
