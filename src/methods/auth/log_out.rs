use crate::Client;
use crate::errors::Result;
use crate::raw::functions;

impl Client {
    /// Terminates the authorization on Telegram's side, forgets the account's cached peers, update state and
    /// transfer connections, and deletes everything the storage engine kept: the `.session` file, unless the client
    /// was built with another [`storage_engine`](crate::ClientBuilder::storage_engine).
    pub async fn log_out(&self) -> Result<()> {
        self.invoke(&functions::auth::LogOut {}).await?;
        self.forget_account();
        Ok(self.0.store.delete().await?)
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
    use crate::storage::{FileStorage, Recording, Storage, UpdateState};

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

    fn client_with(engine: Arc<dyn Storage>, replies: Vec<Result<Vec<u8>>>) -> Client {
        let (mut client, _) = Client::scripted(replies);
        Arc::get_mut(&mut client.0).unwrap().set_store(engine);
        client
    }

    fn stored_client(dir: &Path, replies: Vec<Result<Vec<u8>>>) -> (Client, PathBuf) {
        let _ = std::fs::remove_dir_all(dir);
        std::fs::create_dir_all(dir).unwrap();
        (client_with(Arc::new(FileStorage::new(dir.join("bot"))), replies), dir.join("bot.session"))
    }

    async fn log_in_a(client: &Client) {
        client.set_me(&user(1001));
        let peer = types::InputPeerUser { user_id: 777, access_hash: 5 };
        client.0.peers.lock().unwrap().insert(777, base::InputPeer::InputPeerUser(Box::new(peer)));
        let state = UpdateState { pts: 50, date: 1, ..Default::default() };
        client.save().await.unwrap();
        client.0.save_state(&state).await;
        *client.0.state.lock().unwrap() = Some(state);
    }

    #[tokio::test]
    async fn log_out_forgets_the_account_in_memory_and_on_disk() {
        let dir = std::env::temp_dir().join(format!("vaegram-log-out-forget-{}", std::process::id()));
        let (client, session) = stored_client(&dir, vec![Ok(logged_out())]);
        log_in_a(&client).await;
        let saved = session.exists();

        client.log_out().await.unwrap();
        let me = client.0.me.lock().unwrap().id;
        let peers = client.0.peers.lock().unwrap().len();
        let update_state = client.0.state.lock().unwrap().is_some();
        drop(client);

        let left = session.exists();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(saved);
        assert_eq!((me, peers, update_state), (0, 0, false));
        assert!(!left, "the session file is gone");
    }

    #[tokio::test]
    async fn the_next_account_inherits_nothing_of_the_last() {
        let dir = std::env::temp_dir().join(format!("vaegram-log-out-next-{}", std::process::id()));
        let (client, session) = stored_client(&dir, vec![Ok(logged_out()), Ok(signed_in(2002))]);
        log_in_a(&client).await;
        let running = client.0.state.lock().unwrap().take().unwrap();
        let dispatcher = Dispatcher::resume(client.clone(), running);

        client.log_out().await.unwrap();
        client.sign_in("8801", "hash", "12345").await.unwrap();
        drop(dispatcher);
        let me = client.0.me.lock().unwrap().id;
        let peers = client.0.peers.lock().unwrap().len();
        let update_state = client.0.state.lock().unwrap().is_some();
        client.save().await.unwrap();
        drop(client);

        let left = session.exists();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!((me, peers, update_state), (2002, 0, false));
        assert!(!left, "nothing is written after log_out");
    }

    #[tokio::test]
    async fn log_out_deletes_everything_the_engine_kept() {
        let engine = Arc::new(Recording::default());
        let client = client_with(engine.clone(), vec![Ok(logged_out())]);
        log_in_a(&client).await;
        let saved = (engine.load_session().await.unwrap().is_some(), engine.load_state().await.unwrap().is_some());

        client.log_out().await.unwrap();
        client.save().await.unwrap();
        client.0.save_state_in_background(&UpdateState { pts: 60, date: 2, ..Default::default() });
        drop(client);

        assert_eq!(saved, (true, true));
        let calls = engine.calls();
        let after: Vec<_> = calls.iter().skip_while(|c| **c != "delete").copied().collect();
        assert_eq!(after, ["delete", "close"], "no write after log_out");
        assert!(engine.load_session().await.unwrap().is_none() && engine.export_peers(None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_failing_store_is_an_error_not_a_panic() {
        let client = client_with(Arc::new(Recording::failing()), vec![Ok(logged_out())]);
        let saved = client.save().await;
        let result = client.log_out().await;
        assert!(matches!(saved, Err(Error::Io(_))), "{saved:?}");
        assert!(matches!(result, Err(Error::Io(_))), "{result:?}");
    }
}
