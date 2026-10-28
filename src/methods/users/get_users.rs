use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::{PeerRef, User};
use crate::utils::input_user;

impl Client {
    pub async fn get_users(&self, user_ids: impl IntoIterator<Item = impl Into<PeerRef>>) -> Result<Vec<User>> {
        let users: Vec<PeerRef> = user_ids.into_iter().map(Into::into).collect();
        let mut user_ids = Vec::with_capacity(users.len());
        for user in users {
            user_ids.push(self.peer_id(user).await?);
        }
        let me = self.0.me.lock().unwrap().id;
        let id = {
            let peers = self.0.peers.lock().unwrap();
            // Unknown users go out with access_hash 0, the same fallback resolve_peer uses, so it stays one request.
            let input = |&user_id: &i64| {
                if user_id == me {
                    return base::InputUser::InputUserSelf;
                }
                let unknown = || base::InputUser::InputUser(Box::new(types::InputUser { user_id, access_hash: 0 }));
                peers.get(&user_id).cloned().and_then(input_user).unwrap_or_else(unknown)
            };
            user_ids.iter().map(input).collect()
        };
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
