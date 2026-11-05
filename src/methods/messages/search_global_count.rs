use super::pages::count;
use crate::Client;
use crate::enums::MessagesFilter;
use crate::errors::Result;
use crate::raw::{base, functions};

impl Client {
    /// Counts the messages of all chats that match a text and a kind of message.
    ///
    /// Pass [`MessagesFilter::Empty`] to count any kind.
    pub async fn search_global_count(&self, query: impl Into<String>, filter: MessagesFilter) -> Result<i32> {
        let request = functions::messages::SearchGlobal {
            broadcasts_only: false,
            groups_only: false,
            users_only: false,
            folder_id: None,
            community: None,
            q: query.into(),
            filter: filter.write(),
            min_date: 0,
            max_date: 0,
            offset_rate: 0,
            offset_peer: base::InputPeer::InputPeerEmpty,
            offset_id: 0,
            limit: 1,
        };
        Ok(count(&self.invoke(&request).await?))
    }
}
