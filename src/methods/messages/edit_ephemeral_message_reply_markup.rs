use crate::Client;
use crate::methods::EditEphemeralMessage;
use crate::types::PeerRef;

impl Client {
    /// Without `.reply_markup(…)` on the returned builder, the edit sends no keyboard.
    pub fn edit_ephemeral_message_reply_markup(
        &self,
        chat_id: impl Into<PeerRef>,
        receiver_user_id: impl Into<PeerRef>,
        ephemeral_message_id: i32,
    ) -> EditEphemeralMessage<'_> {
        EditEphemeralMessage::new(self, chat_id.into(), receiver_user_id.into(), ephemeral_message_id)
    }
}
