use super::pages::Pages;
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Message, PeerRef};

#[must_use = "call .next() to fetch messages"]
pub struct GetChatHistory<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    offset_date: i32,
    pages: Pages,
}

impl Client {
    pub fn get_chat_history(&self, chat_id: impl Into<PeerRef>) -> GetChatHistory<'_> {
        GetChatHistory { client: self, chat_id: chat_id.into(), offset_date: 0, pages: Pages::new() }
    }
}

impl GetChatHistory<'_> {
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
        self
    }

    pub fn offset_id(mut self, message_id: i32) -> Self {
        self.pages.offset_id = message_id;
        self
    }

    pub fn offset_date(mut self, unix_time: i32) -> Self {
        self.offset_date = unix_time;
        self
    }

    pub async fn next(&mut self) -> Result<Option<Message>> {
        loop {
            if let Some(message) = self.pages.pop(self.client) {
                return Ok(Some(message));
            }
            let Some(limit) = self.pages.limit() else { return Ok(None) };
            let (chat_id, peer) = self.client.resolve(&self.chat_id).await?;
            self.chat_id = PeerRef::Id(chat_id);
            let request = functions::messages::GetHistory {
                peer,
                offset_id: self.pages.offset_id,
                offset_date: self.offset_date,
                add_offset: 0,
                limit,
                max_id: 0,
                min_id: 0,
                hash: 0,
            };
            let page = self.client.invoke(&request).await?;
            self.pages.fill(self.client, page);
        }
    }
}
