use super::send_options::{SendOptions, send_builder};
use crate::Client;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::{Message, PeerRef};

/// Request built by [`Client::send_contact`]. Await it to send.
#[must_use = "a request does nothing until it is awaited"]
pub struct SendContact<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    media: types::InputMediaContact,
    options: SendOptions,
}

impl Client {
    /// Sends a phone contact card.
    ///
    /// Awaiting the request returns the sent [`Message`].
    pub fn send_contact(
        &self,
        chat_id: impl Into<PeerRef>,
        phone_number: impl Into<String>,
        first_name: impl Into<String>,
    ) -> SendContact<'_> {
        let media = types::InputMediaContact {
            phone_number: phone_number.into(),
            first_name: first_name.into(),
            last_name: String::new(),
            vcard: String::new(),
        };
        SendContact { client: self, chat_id: chat_id.into(), media, options: SendOptions::default() }
    }
}

impl SendContact<'_> {
    /// Last name shown on the contact card. Empty by default.
    pub fn last_name(mut self, last_name: impl Into<String>) -> Self {
        self.media.last_name = last_name.into();
        self
    }

    /// vCard data for the contact. Empty by default.
    pub fn vcard(mut self, vcard: impl Into<String>) -> Self {
        self.media.vcard = vcard.into();
        self
    }

    async fn send(self) -> Result<Message> {
        let media = base::InputMedia::InputMediaContact(Box::new(self.media));
        self.client.send_media(self.chat_id, media, "", None, self.options).await
    }
}

send_builder!(SendContact);
