use crate::Client;
use crate::methods::EditEphemeralMessage;
use crate::types::{MessageText, PeerRef};

impl Client {
    pub fn edit_ephemeral_message_caption(
        &self,
        chat_id: impl Into<PeerRef>,
        receiver_user_id: impl Into<PeerRef>,
        ephemeral_message_id: i32,
        caption: impl Into<String>,
    ) -> EditEphemeralMessage<'_> {
        EditEphemeralMessage::new(self, chat_id.into(), receiver_user_id.into(), ephemeral_message_id)
            .text(MessageText::Text(caption.into()))
    }
}
