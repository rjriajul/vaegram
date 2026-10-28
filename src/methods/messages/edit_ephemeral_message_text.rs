use super::send_options::caption_setters;
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{EphemeralMessage, PeerRef, ReplyMarkup};

#[must_use = "a request does nothing until it is awaited"]
pub struct EditEphemeralMessage<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    receiver_user_id: PeerRef,
    ephemeral_message_id: i32,
    text: Option<String>,
    parse_mode: Option<ParseMode>,
    invert_media: bool,
    reply_markup: Option<ReplyMarkup>,
}

impl Client {
    pub fn edit_ephemeral_message_text(
        &self,
        chat_id: impl Into<PeerRef>,
        receiver_user_id: impl Into<PeerRef>,
        ephemeral_message_id: i32,
        text: impl Into<String>,
    ) -> EditEphemeralMessage<'_> {
        EditEphemeralMessage::new(self, chat_id.into(), receiver_user_id.into(), ephemeral_message_id).text(text.into())
    }
}

impl<'a> EditEphemeralMessage<'a> {
    pub(crate) fn new(client: &'a Client, chat_id: PeerRef, receiver_user_id: PeerRef, id: i32) -> Self {
        EditEphemeralMessage {
            client,
            chat_id,
            receiver_user_id,
            ephemeral_message_id: id,
            text: None,
            parse_mode: None,
            invert_media: false,
            reply_markup: None,
        }
    }

    pub(crate) fn text(mut self, text: String) -> Self {
        self.text = Some(text);
        self
    }

    pub fn show_caption_above_media(mut self) -> Self {
        self.invert_media = true;
        self
    }

    pub fn reply_markup(mut self, markup: impl Into<ReplyMarkup>) -> Self {
        self.reply_markup = Some(markup.into());
        self
    }

    fn request(
        self,
        peer: base::InputPeer,
        receiver_id: base::InputUser,
        message: Option<String>,
        entities: Option<Vec<base::MessageEntity>>,
    ) -> functions::ephemeral::EditMessage {
        functions::ephemeral::EditMessage {
            invert_media: self.invert_media,
            welcome: false,
            peer: Some(peer),
            receiver_id,
            id: self.ephemeral_message_id,
            message,
            media: None,
            entities,
            reply_markup: self.reply_markup.as_ref().map(ReplyMarkup::write),
            rich_message: None,
        }
    }

    async fn send(self) -> Result<EphemeralMessage> {
        let client = self.client;
        let peer = client.resolve(&self.chat_id).await?.1;
        let receiver_id = client.resolve_user(&self.receiver_user_id).await?;
        let (message, entities) = match &self.text {
            Some(text) => {
                let (message, entities) = client.parse_text(text, self.parse_mode).await?;
                (Some(message), (!entities.is_empty()).then_some(entities))
            }
            None => (None, None),
        };
        let updates = client.invoke(&self.request(peer, receiver_id, message, entities)).await?;
        EphemeralMessage::from_updates(client, updates)
    }
}

caption_setters!(EditEphemeralMessage);
into_future!(EditEphemeralMessage, EphemeralMessage);

#[cfg(test)]
mod tests {
    use super::EditEphemeralMessage;
    use crate::Client;
    use crate::raw::{base, functions, types};

    fn request(edit: EditEphemeralMessage<'_>) -> functions::ephemeral::EditMessage {
        let peer = base::InputPeer::InputPeerChat(Box::new(types::InputPeerChat { chat_id: 1 }));
        edit.request(peer, base::InputUser::InputUserSelf, None, None)
    }

    #[test]
    fn edits_markup_without_touching_text() {
        let client = Client::offline();
        let edit = client.edit_ephemeral_message_reply_markup(-1, 1000, 3);
        assert!(edit.text.is_none());
        let request = request(edit);
        assert_eq!((request.message, request.reply_markup, request.invert_media), (None, None, false));
    }
}
