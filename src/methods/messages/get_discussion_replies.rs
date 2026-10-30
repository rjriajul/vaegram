use super::pages::Pages;
use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::types::{Message, PeerRef};

#[must_use = "call .next() to fetch messages"]
pub struct GetDiscussionReplies<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    message_id: i32,
    pages: Pages,
}

impl Client {
    pub fn get_discussion_replies(&self, chat_id: impl Into<PeerRef>, message_id: i32) -> GetDiscussionReplies<'_> {
        GetDiscussionReplies { client: self, chat_id: chat_id.into(), message_id, pages: Pages::new() }
    }
}

impl GetDiscussionReplies<'_> {
    pub fn limit(mut self, limit: usize) -> Self {
        self.pages.remaining = limit;
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
            let request = functions::messages::GetReplies {
                peer,
                msg_id: self.message_id,
                offset_id: self.pages.offset_id,
                offset_date: 0,
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
