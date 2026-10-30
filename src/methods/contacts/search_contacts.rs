use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::types::Chat;

impl Client {
    pub async fn search_contacts(&self, query: &str, limit: i32) -> Result<Vec<Chat>> {
        let request = functions::contacts::Search { broadcasts: false, bots: false, q: query.into(), limit };
        let base::contacts::Found::Found(found) = self.invoke(&request).await?;
        let entities = Entities::new(self, found.users, found.chats);
        Ok(found.my_results.iter().chain(&found.results).map(|peer| Chat::parse(peer, &entities, false)).collect())
    }
}
