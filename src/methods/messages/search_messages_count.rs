use super::pages::count;
use crate::Client;
use crate::enums::MessagesFilter;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::PeerRef;

#[must_use = "a request does nothing until it is awaited"]
pub struct SearchMessagesCount<'a> {
    client: &'a Client,
    chat_id: PeerRef,
    query: String,
    filter: MessagesFilter,
    from_user: Option<PeerRef>,
}

impl Client {
    pub fn search_messages_count(&self, chat_id: impl Into<PeerRef>) -> SearchMessagesCount<'_> {
        SearchMessagesCount {
            client: self,
            chat_id: chat_id.into(),
            query: String::new(),
            filter: MessagesFilter::Empty,
            from_user: None,
        }
    }
}

impl SearchMessagesCount<'_> {
    pub fn query(mut self, query: impl Into<String>) -> Self {
        self.query = query.into();
        self
    }

    pub fn filter(mut self, filter: MessagesFilter) -> Self {
        self.filter = filter;
        self
    }

    pub fn from_user(mut self, user_id: impl Into<PeerRef>) -> Self {
        self.from_user = Some(user_id.into());
        self
    }

    async fn send(self) -> Result<i32> {
        let client = self.client;
        let peer = client.resolve_peer(self.chat_id).await?;
        // The counters call only counts by kind; anything else asks for one match and reads the total.
        if self.query.is_empty() && self.from_user.is_none() && self.filter != MessagesFilter::Empty {
            let request = functions::messages::GetSearchCounters {
                peer,
                saved_peer_id: None,
                top_msg_id: None,
                filters: vec![self.filter.write()],
            };
            let counters = client.invoke(&request).await?;
            return Ok(counters.iter().map(|base::messages::SearchCounter::SearchCounter(c)| c.count).sum());
        }
        let from_id = match self.from_user {
            Some(user_id) => Some(client.resolve_peer(user_id).await?),
            None => None,
        };
        let request = functions::messages::Search {
            peer,
            q: self.query,
            from_id,
            saved_peer_id: None,
            saved_reaction: None,
            top_msg_id: None,
            filter: self.filter.write(),
            min_date: 0,
            max_date: 0,
            offset_id: 0,
            add_offset: 0,
            limit: 1,
            max_id: 0,
            min_id: 0,
            hash: 0,
        };
        Ok(count(&client.invoke(&request).await?))
    }
}

into_future!(SearchMessagesCount, i32);
