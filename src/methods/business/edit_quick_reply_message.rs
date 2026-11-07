use super::add_quick_reply_message::quick_reply_message;
use crate::Client;
use crate::enums::ParseMode;
use crate::errors::Result;
use crate::methods::messages::caption_setters;
use crate::raw::{base, functions};
use crate::types::{Message, Text};

/// Request built by [`Client::edit_quick_reply_message`]. Awaiting it returns the edited [`Message`].
#[must_use = "a request does nothing until it is awaited"]
pub struct EditQuickReplyMessage<'a> {
    client: &'a Client,
    shortcut_id: i32,
    message_id: i32,
    text: Text,
    parse_mode: Option<ParseMode>,
}

impl Client {
    /// Replaces the text of message `message_id` in the quick reply shortcut `shortcut_id`.
    ///
    /// Message ids come from [`get_quick_reply_messages`](Client::get_quick_reply_messages). Awaiting the request
    /// returns the edited [`Message`].
    pub fn edit_quick_reply_message(
        &self,
        shortcut_id: i32,
        message_id: i32,
        text: impl Into<Text>,
    ) -> EditQuickReplyMessage<'_> {
        EditQuickReplyMessage { client: self, shortcut_id, message_id, text: text.into(), parse_mode: None }
    }
}

impl EditQuickReplyMessage<'_> {
    async fn send(self) -> Result<Message> {
        let (message, entities) = self.client.write_text(self.text, self.parse_mode).await?;
        let request = functions::messages::EditMessage {
            no_webpage: false,
            invert_media: false,
            // Quick reply messages live in the account's own chat.
            peer: base::InputPeer::InputPeerSelf,
            id: self.message_id,
            message: Some(message),
            media: None,
            reply_markup: None,
            entities: (!entities.is_empty()).then_some(entities),
            schedule_date: None,
            schedule_repeat_period: None,
            quick_reply_shortcut_id: Some(self.shortcut_id),
            rich_message: None,
        };
        quick_reply_message(self.client, self.client.invoke(&request).await?)
    }
}

caption_setters!(EditQuickReplyMessage);
into_future!(EditQuickReplyMessage, Message);
