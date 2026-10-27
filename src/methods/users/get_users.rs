use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::User;

impl Client {
    pub async fn get_users(&self, user_ids: &[i64]) -> Result<Vec<User>> {
        let me = self.0.me.lock().unwrap().id;
        let mut id = Vec::with_capacity(user_ids.len());
        {
            let peers = self.0.peers.lock().unwrap();
            for &user_id in user_ids {
                if user_id == me {
                    id.push(base::InputUser::InputUserSelf);
                    continue;
                }
                // Unknown users go out with access_hash 0, the same fallback resolve_peer uses.
                let access_hash = match peers.get(&user_id) {
                    Some(base::InputPeer::InputPeerUser(p)) => p.access_hash,
                    _ => 0,
                };
                id.push(base::InputUser::InputUser(Box::new(types::InputUser { user_id, access_hash })));
            }
        }
        let users = self.invoke(&functions::users::GetUsers { id }).await?;
        let entities = Entities::new(self, users, Vec::new());
        user_ids
            .iter()
            .map(|&id| {
                if entities.users.contains_key(&id) {
                    Ok(User::parse(id, &entities))
                } else {
                    Err(Error::PeerNotFound(id))
                }
            })
            .collect()
    }
}
