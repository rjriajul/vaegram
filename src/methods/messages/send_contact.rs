use super::send_options::{SendOptions, send_builder};
use crate::Client;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::Message;

#[must_use = "a request does nothing until it is awaited"]
pub struct SendContact<'a> {
    client: &'a Client,
    chat_id: i64,
    media: types::InputMediaContact,
    options: SendOptions,
}

impl Client {
    pub fn send_contact(
        &self,
        chat_id: i64,
        phone_number: impl Into<String>,
        first_name: impl Into<String>,
    ) -> SendContact<'_> {
        let media = types::InputMediaContact {
            phone_number: phone_number.into(),
            first_name: first_name.into(),
            last_name: String::new(),
            vcard: String::new(),
        };
        SendContact { client: self, chat_id, media, options: SendOptions::default() }
    }
}

impl SendContact<'_> {
    pub fn last_name(mut self, last_name: impl Into<String>) -> Self {
        self.media.last_name = last_name.into();
        self
    }

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
