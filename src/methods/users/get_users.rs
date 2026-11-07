use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::{Error, Result};
use crate::raw::{base, functions, types};
use crate::types::{PeerRef, User};
use crate::utils::input_user;

impl Client {
    /// Returns users by id, in the order given.
    ///
    /// Fails with `PeerNotFound` if Telegram returns no user for one of the ids. Users the client hasn't seen
    /// are still requested, with an access hash of 0.
    pub async fn get_users(&self, user_ids: impl IntoIterator<Item = impl Into<PeerRef>>) -> Result<Vec<User>> {
        let users: Vec<PeerRef> = user_ids.into_iter().map(Into::into).collect();
        let mut user_ids = Vec::with_capacity(users.len());
        for user in users {
            user_ids.push(self.peer_id(user).await?);
        }
        let mut id = Vec::with_capacity(user_ids.len());
        for &user_id in &user_ids {
            // Unknown users go out with access_hash 0, the same fallback resolve_peer uses, so it stays one request.
            let known = self.known_peer(user_id).await.and_then(input_user);
            id.push(
                known.unwrap_or_else(|| {
                    base::InputUser::InputUser(Box::new(types::InputUser { user_id, access_hash: 0 }))
                }),
            );
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::Client;
    use crate::errors::Error;
    use crate::raw::{Serializable, base, functions, types};
    use crate::storage::{MemoryStorage, PeerRecord, Storage};

    #[tokio::test]
    async fn a_user_saved_in_an_earlier_run_goes_out_with_its_hash() {
        let engine = Arc::new(MemoryStorage::new());
        engine.update_peers(&[PeerRecord::new(777, 5)]).await.unwrap();
        let (mut client, mut sent) = Client::scripted(vec![Err(Error::Unexpected("offline"))]);
        Arc::get_mut(&mut client.0).unwrap().set_store(engine);
        let _ = client.get_users([777, 778]).await;
        let user =
            |user_id, access_hash| base::InputUser::InputUser(Box::new(types::InputUser { user_id, access_hash }));
        let expected = functions::users::GetUsers { id: vec![user(777, 5), user(778, 0)] };
        assert_eq!(sent.try_recv().unwrap(), expected.to_bytes());
    }
}
