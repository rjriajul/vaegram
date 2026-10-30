use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions};
use crate::utils::get_peer_id;

impl Client {
    pub async fn resolve_phone(&self, phone_number: &str) -> Result<i64> {
        let phone = phone_number.trim_start_matches('+').replace([' ', '-'], "");
        let base::contacts::ResolvedPeer::ResolvedPeer(resolved) =
            self.invoke(&functions::contacts::ResolvePhone { phone }).await?;
        Entities::new(self, resolved.users, resolved.chats);
        Ok(get_peer_id(&resolved.peer))
    }
}
