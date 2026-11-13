use std::fmt;

use crate::Client;
use crate::methods::{EditInlineReplyMarkup, EditInlineText};
use crate::types::{InlineKeyboardMarkup, InlineMessageId, MessageText};

/// The message sent in answer to a guest query by `GuestMessage::answer` and its shortcuts.
///
/// It can be edited afterwards through [`edit_text`](SentGuestMessage::edit_text) and
/// [`edit_reply_markup`](SentGuestMessage::edit_reply_markup).
#[derive(Clone)]
#[non_exhaustive]
pub struct SentGuestMessage {
    /// Id of the sent message.
    pub inline_message_id: InlineMessageId,
    client: Client,
}

impl SentGuestMessage {
    pub(crate) fn new(client: Client, inline_message_id: InlineMessageId) -> Self {
        SentGuestMessage { inline_message_id, client }
    }

    /// Returns the client the message was sent from.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Replaces the text of the sent message, the way [`Client::edit_inline_text`] does.
    pub fn edit_text(&self, text: impl Into<MessageText>) -> EditInlineText<'_> {
        self.client.edit_inline_text(self.inline_message_id.clone(), text)
    }

    /// Replaces the inline keyboard of the sent message. `None` removes the keyboard.
    pub fn edit_reply_markup(&self, markup: Option<InlineKeyboardMarkup>) -> EditInlineReplyMarkup<'_> {
        self.client.edit_inline_reply_markup(self.inline_message_id.clone(), markup)
    }
}

impl fmt::Debug for SentGuestMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SentGuestMessage").field("inline_message_id", &self.inline_message_id).finish_non_exhaustive()
    }
}

// Two handles name the same message when their inline message ids match; the client they came from doesn't matter.
impl PartialEq for SentGuestMessage {
    fn eq(&self, other: &Self) -> bool {
        self.inline_message_id == other.inline_message_id
    }
}

#[cfg(test)]
mod tests {
    use super::SentGuestMessage;
    use crate::Client;
    use crate::raw::{Identifiable, base, functions, types};
    use crate::types::InlineMessageId;

    fn id() -> InlineMessageId {
        let id = types::InputBotInlineMessageID64 { dc_id: 2, owner_id: 5, id: 6, access_hash: 7 };
        InlineMessageId(base::InputBotInlineMessageID::InputBotInlineMessageID64(Box::new(id)))
    }

    #[tokio::test]
    async fn edits_the_sent_message() {
        let (client, mut rx) = Client::recording();
        let sent = SentGuestMessage::new(client, id());
        let _ = sent.edit_text("hi").await;
        let body = rx.try_recv().expect("no request sent");
        let edit = <functions::messages::EditInlineBotMessage as Identifiable>::CONSTRUCTOR_ID;
        assert_eq!(u32::from_le_bytes(body[..4].try_into().unwrap()), edit);
        let _ = sent.edit_reply_markup(None).await;
        let body = rx.try_recv().expect("no request sent");
        assert_eq!(u32::from_le_bytes(body[..4].try_into().unwrap()), edit);
    }
}
