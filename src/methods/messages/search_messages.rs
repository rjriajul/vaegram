use super::pages::Pages;
use crate::Client;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::{Message, PeerRef};

#[must_use = "call .next() to fetch messages"]
pub struct SearchMessages<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    query: String,
    from_user: Option<PeerRef>,
    pages: Pages,
}

impl Client {
    pub fn search_messages(&self, chat_id: impl Into<PeerRef>) -> SearchMessages<'_> {
        SearchMessages {
            client: self,
            chat_id: chat_id.into(),
            query: String::new(),
            from_user: None,
            pages: Pages::new(),
        }
    }
}

impl SearchMessages<'_> {
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
        self
    }

    pub fn from_user(mut self, user_id: impl Into<PeerRef>) -> Self {
        self.from_user = Some(user_id.into());
        self
    }

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
            let from_id = match &self.from_user {
                Some(user) => {
                    let (user_id, peer) = self.client.resolve(user).await?;
                    self.from_user = Some(PeerRef::Id(user_id));
                    Some(peer)
                }
                None => None,
            };
            let (chat_id, peer) = self.client.resolve(&self.chat_id).await?;
            self.chat_id = PeerRef::Id(chat_id);
            let request = functions::messages::Search {
                peer,
                q: self.query.clone(),
                from_id,
                saved_peer_id: None,
                saved_reaction: None,
                top_msg_id: None,
                filter: base::MessagesFilter::InputMessagesFilterEmpty,
                min_date: 0,
                max_date: 0,
                offset_id: self.pages.offset_id,
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
