use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Terminates the authorization on Telegram's side, forgets the account's cached peers, update state and
    /// transfer connections, and deletes the saved session and state: the local files, unless the client was built
    /// with a [`store`](crate::ClientBuilder::store).
    pub async fn log_out(&self) -> Result<()> {
        self.invoke(&functions::auth::LogOut {}).await?;
        self.forget_account();
        Ok(self.0.store.delete()?)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use crate::Client;
    use crate::dispatcher::Dispatcher;
    use crate::errors::{Error, Result};
    use crate::raw::{Serializable, base, types};
    use crate::storage::state::UpdateState;
    use crate::storage::{FileStore, Recording, Store};

    fn logged_out() -> Vec<u8> {
        base::auth::LoggedOut::LoggedOut(Box::new(types::auth::LoggedOut { future_auth_token: None })).to_bytes()
    }

    fn user(id: i64) -> base::User {
        let mut buf = Vec::new();
        0u32.serialize(&mut buf);
        0u32.serialize(&mut buf);
        id.serialize(&mut buf);
        base::User::User(Box::new(types::User::deserialize_bare(&mut &buf[..]).unwrap()))
    }

    fn signed_in(id: i64) -> Vec<u8> {
        let auth = types::auth::Authorization {
            setup_password_required: false,
            otherwise_relogin_days: None,
            tmp_sessions: None,
            future_auth_token: None,
            user: user(id),
        };
        base::auth::Authorization::Authorization(Box::new(auth)).to_bytes()
    }

    fn stored_client(dir: &Path, replies: Vec<Result<Vec<u8>>>) -> (Client, PathBuf, PathBuf) {
        std::fs::create_dir_all(dir).unwrap();
        let (session, state) = (dir.join("bot.session"), dir.join("bot.state"));
        let (mut client, _) = Client::scripted(replies);
        let inner = Arc::get_mut(&mut client.0).unwrap();
        inner.store = Arc::new(Store::new(FileStore::new(dir.join("bot"))));
        (client, session, state)
    }

    async fn log_in_a(client: &Client) {
        client.set_me(&user(1001));
        let peer = types::InputPeerUser { user_id: 777, access_hash: 5 };
        client.0.peers.lock().unwrap().insert(777, base::InputPeer::InputPeerUser(Box::new(peer)));
        let state = UpdateState { pts: 50, date: 1, ..Default::default() };
        client.save().unwrap();
        client.0.save_state(&state).await;
        *client.0.state.lock().unwrap() = Some(state);
    }

    #[tokio::test]
    async fn log_out_forgets_the_account_in_memory_and_on_disk() {
        let dir = std::env::temp_dir().join(format!("vaegram-log-out-forget-{}", std::process::id()));
        let (client, session, state) = stored_client(&dir, vec![Ok(logged_out())]);
        log_in_a(&client).await;
        let saved = state.exists();

        client.log_out().await.unwrap();
        let me = client.0.me.lock().unwrap().id;
        let peers = client.0.peers.lock().unwrap().len();
        let update_state = client.0.state.lock().unwrap().is_some();
        drop(client);

        let left = (session.exists(), state.exists());
        let _ = std::fs::remove_dir_all(&dir);
        assert!(saved);
        assert_eq!((me, peers, update_state), (0, 0, false));
        assert_eq!(left, (false, false));
    }

    #[tokio::test]
    async fn the_next_account_inherits_nothing_of_the_last() {
        let dir = std::env::temp_dir().join(format!("vaegram-log-out-next-{}", std::process::id()));
        let (client, session, state) = stored_client(&dir, vec![Ok(logged_out()), Ok(signed_in(2002))]);
        log_in_a(&client).await;
        let running = client.0.state.lock().unwrap().take().unwrap();
        let dispatcher = Dispatcher::resume(client.clone(), running);

        client.log_out().await.unwrap();
        client.sign_in("8801", "hash", "12345").await.unwrap();
        drop(dispatcher);
        let me = client.0.me.lock().unwrap().id;
        let peers = client.0.peers.lock().unwrap().len();
        let update_state = client.0.state.lock().unwrap().is_some();
        client.save().unwrap();
        drop(client);

        let left = (session.exists(), state.exists());
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!((me, peers, update_state), (2002, 0, false));
        assert_eq!(left, (false, false));
    }

    #[tokio::test]
    async fn log_out_leaves_no_session_or_state_behind() {
        let dir = std::env::temp_dir().join(format!("vaegram-log-out-{}", std::process::id()));
        let (client, session, state) = stored_client(&dir, vec![Ok(logged_out())]);
        client.save().unwrap();
        client.0.save_state_in_background(&Default::default());

        client.log_out().await.unwrap();
        client.save().unwrap();
        drop(client);

        let left = (session.exists(), state.exists());
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(left, (false, false));
    }

    #[tokio::test]
    async fn failed_session_delete_still_removes_state() {
        let dir = std::env::temp_dir().join(format!("vaegram-log-out-err-{}", std::process::id()));
        let (client, session, state) = stored_client(&dir, vec![Ok(logged_out())]);
        std::fs::create_dir_all(&session).unwrap();
        std::fs::write(&state, b"old").unwrap();

        let result = client.log_out().await;
        drop(client);

        let left = state.exists();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_err());
        assert!(!left);
    }

    fn client_with(store: Arc<Recording>, replies: Vec<Result<Vec<u8>>>) -> Client {
        let (mut client, _) = Client::scripted(replies);
        Arc::get_mut(&mut client.0).unwrap().store = Arc::new(Store::new(store));
        client
    }

    #[tokio::test]
    async fn log_out_deletes_both_keys_from_a_custom_store() {
        let store = Arc::new(Recording::default());
        let client = client_with(store.clone(), vec![Ok(logged_out())]);
        log_in_a(&client).await;
        let saved = (store.value("session").is_some(), store.value("state").is_some());

        client.log_out().await.unwrap();
        client.save().unwrap();
        drop(client);

        assert_eq!(saved, (true, true));
        assert_eq!(store.calls(), ["save session", "save state", "delete session", "delete state"]);
        assert_eq!((store.value("session"), store.value("state")), (None, None));
    }

    #[tokio::test]
    async fn a_failing_store_is_an_error_not_a_panic() {
        let store = Arc::new(Recording::failing());
        let client = client_with(store.clone(), vec![Ok(logged_out())]);
        let saved = client.save();
        let result = client.log_out().await;
        assert!(matches!(saved, Err(Error::Io(_))), "{saved:?}");
        assert!(matches!(result, Err(Error::Io(_))), "{result:?}");
        assert_eq!(store.calls(), ["save session", "delete session", "delete state"]);
    }
}
