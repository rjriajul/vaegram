use super::send_options::{SendOptions, send_builder};
use crate::Client;
use crate::errors::Result;
use crate::raw::{base, types};
use crate::types::{Message, PeerRef};

#[must_use = "a request does nothing until it is awaited"]
pub struct SendDice<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    media: types::InputMediaDice,
    options: SendOptions,
}

impl Client {
    pub fn send_dice(&self, chat_id: impl Into<PeerRef>) -> SendDice<'_> {
        let media = types::InputMediaDice { emoticon: "🎲".into() };
        SendDice { client: self, chat_id: chat_id.into(), media, options: SendOptions::default() }
    }
}

impl SendDice<'_> {
    pub fn emoji(mut self, emoji: impl Into<String>) -> Self {
        self.media.emoticon = emoji.into();
        self
    }

    async fn send(self) -> Result<Message> {
        let media = base::InputMedia::InputMediaDice(Box::new(self.media));
        self.client.send_media(self.chat_id, media, "", None, self.options).await
    }
}

send_builder!(SendDice);
