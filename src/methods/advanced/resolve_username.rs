use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::utils::get_peer_id;

impl Client {
    /// Always asks Telegram. Chat arguments given as `@username` use the stored peer instead while the name was
    /// seen less than 8 hours ago.
    pub async fn resolve_username(&self, username: &str) -> Result<i64> {
        let request =
            functions::contacts::ResolveUsername { username: username.trim_start_matches('@').into(), referer: None };
        let base::contacts::ResolvedPeer::ResolvedPeer(resolved) = self.invoke(&request).await?;
        Entities::new(self, resolved.users, resolved.chats);
        Ok(get_peer_id(&resolved.peer))
    }
}

#[cfg(test)]
mod tests {
    use crate::Client;
    use crate::errors::Error;
    use crate::raw::Identifiable;
    use crate::raw::functions::contacts;

    #[tokio::test]
    async fn resolve_username_always_asks() {
        let (client, mut sent) = Client::scripted(vec![Err(Error::Unexpected("offline"))]);
        let mut fresh = crate::storage::PeerRecord::new(100, 7);
        (fresh.usernames, fresh.updated) = (vec!["alice".into()], i64::MAX);
        client.0.peers.lock().unwrap().load([fresh]);
        let result = client.resolve_username("@alice").await;
        assert!(matches!(result, Err(Error::Unexpected("offline"))), "{result:?}");
        let body = sent.try_recv().expect("no request sent");
        assert_eq!(u32::from_le_bytes(body[..4].try_into().unwrap()), contacts::ResolveUsername::CONSTRUCTOR_ID);
    }
}
