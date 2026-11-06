use crate::Client;
use crate::errors::Result;
use crate::methods::pages::wire_limit;
use crate::raw::functions;
use crate::types::FoundContacts;

impl Client {
    /// Searches your contacts and public chats by name or username.
    ///
    /// Returns at most `limit` matches, split into [`FoundContacts::my_results`] and
    /// [`FoundContacts::global_results`].
    pub async fn search_contacts(&self, query: &str, limit: usize) -> Result<FoundContacts> {
        let limit = wire_limit(limit);
        let request = functions::contacts::Search { broadcasts: false, bots: false, q: query.into(), limit };
        Ok(FoundContacts::parse(self, self.invoke(&request).await?))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::raw::{Serializable, types};

    fn found(my_results: Vec<i64>, results: Vec<i64>) -> Vec<u8> {
        use crate::raw::base::{Peer, contacts::Found};
        let peers =
            |ids: Vec<i64>| ids.into_iter().map(|user_id| Peer::PeerUser(types::PeerUser { user_id })).collect();
        let found = types::contacts::Found {
            my_results: peers(my_results),
            results: peers(results),
            chats: vec![],
            users: vec![],
        };
        Found::Found(Box::new(found)).to_bytes()
    }

    #[tokio::test]
    async fn search_keeps_my_and_global_results_apart() {
        let (client, _rx) = Client::scripted(vec![Ok(found(vec![1, 2], vec![3]))]);
        let result = client.search_contacts("cat", 10).await.unwrap();
        let ids = |chats: &[crate::types::Chat]| chats.iter().map(|c| c.id).collect::<Vec<_>>();
        assert_eq!(ids(&result.my_results), [1, 2]);
        assert_eq!(ids(&result.global_results), [3]);
    }
}
