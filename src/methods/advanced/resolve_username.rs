use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::utils::get_peer_id;

impl Client {
    pub async fn resolve_username(&self, username: &str) -> Result<i64> {
        let request =
            functions::contacts::ResolveUsername { username: username.trim_start_matches('@').into(), referer: None };
        let base::contacts::ResolvedPeer::ResolvedPeer(resolved) = self.invoke(&request).await?;
        Entities::new(self, resolved.users, resolved.chats);
        Ok(get_peer_id(&resolved.peer))
    }
}
