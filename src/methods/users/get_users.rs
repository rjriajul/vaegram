use crate::Client;
use crate::dispatcher::Entities;
use crate::errors::Result;
use crate::raw::{base, functions, types};
use crate::types::{PeerRef, User};
use crate::utils::input_user;

impl Client {
    /// Returns users by id, one slot per id in the order given: ids Telegram has no user for are `None`.
    ///
    /// Users the client hasn't seen are still requested, with an access hash of 0. Compare
    /// [`get_messages`](Client::get_messages), which has the same shape.
    pub async fn get_users(&self, user_ids: impl IntoIterator<Item = impl Into<PeerRef>>) -> Result<Vec<Option<User>>> {
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
        Ok(user_ids.iter().map(|&id| entities.users.contains_key(&id).then(|| User::parse(id, &entities))).collect())
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

    // Telegram answers with the users it has, so a requested id it doesn't know keeps its place as `None`: the
    // result lines up with the ids given, as `get_messages` does with message ids.
    #[tokio::test]
    async fn an_unknown_id_is_none_in_its_slot() {
        let found = vec![user(777)];
        let (client, _sent) = Client::scripted(vec![Ok(found.to_bytes())]);
        let users = client.get_users([777, 778]).await.unwrap();
        let ids: Vec<Option<i64>> = users.iter().map(|user| user.as_ref().map(|user| user.id)).collect();
        assert_eq!(ids, [Some(777), None]);
    }

    // A bare user with no flags set: only its id is on the wire.
    fn user(id: i64) -> base::User {
        let mut buf = Vec::new();
        0u32.serialize(&mut buf);
        0u32.serialize(&mut buf);
        id.serialize(&mut buf);
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }
}
