use crate::Client;
use crate::crypto::random;
use crate::dispatcher::Entities;
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::methods::messages::caption_setters;
use crate::raw::{base, functions, types};
use crate::types::{Message, Text};

/// Request built by [`Client::add_quick_reply_message`]. Awaiting it returns the added [`Message`].
#[must_use = "a request does nothing until it is awaited"]
pub struct AddQuickReplyMessage<'a> {
    client: &'a Client,
    shortcut: String,
    text: Text,
    parse_mode: Option<ParseMode>,
}

impl Client {
    /// Adds a text message to the end of the quick reply shortcut named `shortcut`. The first message added under a
    /// new name creates the shortcut.
    ///
    /// Awaiting the request returns the added [`Message`]; its
    /// [`quick_reply_shortcut_id`](Message::quick_reply_shortcut_id) is the id that the other quick reply methods
    /// and [`set_business_greeting_message`](Client::set_business_greeting_message) take. Fails with
    /// `Error::InvalidArgument` if `shortcut` is empty, and with `QUICK_REPLIES_TOO_MUCH` or
    /// `REPLY_MESSAGES_TOO_MUCH` once the account's limit of shortcuts or of messages per shortcut is reached;
    /// [`check_quick_reply_shortcut`](Client::check_quick_reply_shortcut) tells beforehand.
    pub fn add_quick_reply_message(
        &self,
        shortcut: impl Into<String>,
        text: impl Into<Text>,
    ) -> AddQuickReplyMessage<'_> {
        AddQuickReplyMessage { client: self, shortcut: shortcut.into(), text: text.into(), parse_mode: None }
    }
}

impl AddQuickReplyMessage<'_> {
    async fn send(self) -> Result<Message> {
        if self.shortcut.is_empty() {
            return Err(Error::InvalidArgument("a quick reply shortcut needs a name"));
        }
        let (message, entities) = self.client.write_text(self.text, self.parse_mode).await?;
        let shortcut = Box::new(types::InputQuickReplyShortcut { shortcut: self.shortcut });
        let request = functions::messages::SendMessage {
            no_webpage: false,
            silent: false,
            background: false,
            clear_draft: false,
            noforwards: false,
            update_stickersets_order: false,
            invert_media: false,
            allow_paid_floodskip: false,
            // Quick reply messages always go to the account's own chat.
            peer: base::InputPeer::InputPeerSelf,
            reply_to: None,
            message,
            // Telegram drops a second message with the same random_id, so retries can't double-send.
            random_id: i64::from_le_bytes(random()),
            reply_markup: None,
            entities: (!entities.is_empty()).then_some(entities),
            schedule_date: None,
            schedule_repeat_period: None,
            send_as: None,
            quick_reply_shortcut: Some(base::InputQuickReplyShortcut::InputQuickReplyShortcut(shortcut)),
            effect: None,
            allow_paid_stars: None,
            suggested_post: None,
            rich_message: None,
        };
        quick_reply_message(self.client, self.client.invoke(&request).await?)
    }
}

/// The quick reply message Telegram answers a send or an edit with.
pub(super) fn quick_reply_message(client: &Client, updates: base::Updates) -> Result<Message> {
    let (updates, users, chats) = match updates {
        base::Updates::Updates(u) => (u.updates, u.users, u.chats),
        base::Updates::UpdatesCombined(u) => (u.updates, u.users, u.chats),
        base::Updates::UpdateShort(u) => (vec![u.update], Vec::new(), Vec::new()),
        _ => Default::default(),
    };
    let entities = Entities::new(client, users, chats);
    // Not cached: quick reply ids are their own sequence and would clash with the own chat's message ids.
    updates
        .into_iter()
        .find_map(|u| match u {
            base::Update::UpdateQuickReplyMessage(u) => Message::from_raw(client, u.message, &entities),
            _ => None,
        })
        .ok_or(Error::Unexpected("Telegram sent no quick reply message back"))
}

caption_setters!(AddQuickReplyMessage);
into_future!(AddQuickReplyMessage, Message);
