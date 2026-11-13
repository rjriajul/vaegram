use std::fmt;

use crate::Client;
use crate::methods::{EditInlineReplyMarkup, EditInlineText};
use crate::types::{InlineKeyboardMarkup, InlineMessageId, MessageText};

/// The message sent in answer to a guest query by `GuestMessage::answer` and its shortcuts.
///
/// The answer is an inline message: [`edit_text`](SentGuestMessage::edit_text) and
/// [`edit_reply_markup`](SentGuestMessage::edit_reply_markup) edit it through `messages.editInlineBotMessage`, the
/// way [`Client::edit_inline_text`] and [`Client::edit_inline_reply_markup`] do.
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
    pub fn edit_reply_markup(&self, reply_markup: Option<InlineKeyboardMarkup>) -> EditInlineReplyMarkup<'_> {
        self.client.edit_inline_reply_markup(self.inline_message_id.clone(), reply_markup)
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
    use crate::raw::{Serializable, base, functions, types};
    use crate::types::InlineMessageId;

    fn raw_id() -> base::InputBotInlineMessageID {
        let id = types::InputBotInlineMessageID64 { dc_id: 2, owner_id: 5, id: 6, access_hash: 7 };
        base::InputBotInlineMessageID::InputBotInlineMessageID64(Box::new(id))
    }

    fn edit(message: Option<&str>) -> Vec<u8> {
        functions::messages::EditInlineBotMessage {
            no_webpage: false,
            invert_media: false,
            id: raw_id(),
            message: message.map(String::from),
            media: None,
            reply_markup: None,
            entities: None,
            rich_message: None,
        }
        .to_bytes()
    }

    #[tokio::test]
    async fn edit_text_edits_the_inline_message() {
        let (client, mut rx) = Client::recording();
        let sent = SentGuestMessage::new(client, InlineMessageId(raw_id()));
        let _ = sent.edit_text("hi").await;
        assert_eq!(rx.try_recv().expect("no request sent"), edit(Some("hi")));
    }

    #[tokio::test]
    async fn edit_reply_markup_edits_the_inline_message() {
        let (client, mut rx) = Client::recording();
        let sent = SentGuestMessage::new(client, InlineMessageId(raw_id()));
        let _ = sent.edit_reply_markup(None).await;
        assert_eq!(rx.try_recv().expect("no request sent"), edit(None));
    }

    #[test]
    fn handles_compare_by_id() {
        let (one, two) = (Client::offline(), Client::offline());
        let sent = SentGuestMessage::new(one, InlineMessageId(raw_id()));
        assert_eq!(sent, SentGuestMessage::new(two, InlineMessageId(raw_id())));
        assert!(format!("{sent:?}").starts_with("SentGuestMessage { inline_message_id"));
    }
}
