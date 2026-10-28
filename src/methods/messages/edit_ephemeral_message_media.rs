use crate::Client;
use crate::methods::EditEphemeralMessage;
use crate::types::{InputMedia, PeerRef};

impl Client {
    pub fn edit_ephemeral_message_media<'a>(
        &'a self,
        chat_id: impl Into<PeerRef>,
        receiver_user_id: impl Into<PeerRef>,
        ephemeral_message_id: i32,
        media: InputMedia<'a>,
    ) -> EditEphemeralMessage<'a> {
        EditEphemeralMessage::new(self, chat_id.into(), receiver_user_id.into(), ephemeral_message_id).media(media)
    }
}
