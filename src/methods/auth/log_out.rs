use crate::Client;
use crate::errors::Result;
use crate::raw::functions;
use crate::storage;

impl Client {
    /// Terminates the authorization on Telegram's side and deletes the local session and state files.
    pub async fn log_out(&self) -> Result<()> {
        self.invoke(&functions::auth::LogOut {}).await?;
        let path = self.0.storage_path.lock().unwrap().take();
        let session = path.map_or(Ok(()), |path| storage::delete(&path));
        let state = self.0.state_file.as_ref().map_or(Ok(()), |file| file.delete());
        session?;
        Ok(state?)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use crate::Client;
    use crate::raw::{Serializable, base, types};
    use crate::storage::state::StateFile;

    fn stored_client(dir: &Path) -> (Client, PathBuf, PathBuf) {
        std::fs::create_dir_all(dir).unwrap();
        let (session, state) = (dir.join("bot.session"), dir.join("bot.state"));
        let logged_out = base::auth::LoggedOut::LoggedOut(Box::new(types::auth::LoggedOut { future_auth_token: None }));
        let (mut client, _) = Client::scripted(vec![Ok(logged_out.to_bytes())]);
        let inner = Arc::get_mut(&mut client.0).unwrap();
        *inner.storage_path.get_mut().unwrap() = Some(session.clone());
        inner.state_file = Some(Arc::new(StateFile::new(state.clone())));
        (client, session, state)
    }

    #[tokio::test]
    async fn log_out_leaves_no_session_or_state_behind() {
        let dir = std::env::temp_dir().join(format!("vaegram-log-out-{}", std::process::id()));
        let (client, session, state) = stored_client(&dir);
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
        let (client, session, state) = stored_client(&dir);
        std::fs::create_dir_all(&session).unwrap();
        std::fs::write(&state, b"old").unwrap();

        let result = client.log_out().await;
        drop(client);

        let left = state.exists();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_err());
        assert!(!left);
    }
}
