use std::sync::atomic::Ordering;

use crate::Client;

impl Client {
    /// Closes every connection for good. `run` returns, and requests waiting or made later fail with
    /// `Error::Disconnected`, on every data center; so does `restart`. Build a new client to connect again.
    ///
    /// It doesn't wait for `run`, so a handler can call it. To let the running handlers finish first: `stop`,
    /// await `run`, then `disconnect`.
    ///
    /// The storage is saved and closed here, or as the running `run` returns, even while clones of the client live,
    /// so a new client can open the same session file. Nothing is read from or written to it after that.
    pub fn disconnect(&self) {
        self.stop();
        self.session().close();
        self.0.transfers.shut();
        // SeqCst on both sides: `run` ending clears `running`, then checks this, so one of the two closes it.
        self.0.disconnected.store(true, Ordering::SeqCst);
        if !self.0.running.load(Ordering::SeqCst) {
            self.0.finish_store();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::Client;
    use crate::errors::Error;
    use crate::raw::functions;
    use crate::session::test_dc::TestDc;
    use crate::storage::{FileStorage, Storage, UpdateState};

    // A handler holding a clone of its own client keeps the client alive for ever, so the file can't wait for the
    // last clone to go.
    #[tokio::test]
    async fn disconnect_lets_go_of_the_session_file_while_a_clone_lives() {
        let dir = std::env::temp_dir().join(format!("vaegram-disconnect-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (mut client, _sent) = Client::scripted(Vec::new());
        Arc::get_mut(&mut client.0).unwrap().set_store(Arc::new(FileStorage::new(dir.join("bot"))));
        client.0.me.lock().unwrap().id = 1001;
        client.save().await.unwrap();
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 50, date: 1, ..Default::default() });
        let clone = client.clone();

        let second = FileStorage::new(dir.join("bot"));
        let while_live = second.open().await.err().map(|e| e.kind());
        client.disconnect();
        let after_disconnect = second.open().await;
        let saved = second.load_state().await.unwrap().and_then(|s| s.update).map(|u| u.pts);
        drop((client, clone));
        drop(second);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(while_live, Some(std::io::ErrorKind::ResourceBusy), "two live clients on one file");
        assert!(after_disconnect.is_ok(), "{after_disconnect:?}");
        assert_eq!(saved, Some(50), "the update state is saved before the file is let go");
    }

    #[tokio::test]
    async fn disconnect_ends_every_connection() {
        let dc = TestDc::start([5; 256], false).await;
        let client = dc.client([5; 256]).await;
        let ping = functions::Ping { ping_id: 1 };
        assert!(client.invoke(&ping).await.is_ok());
        let lease = client.transfer_sessions(client.dc(4).unwrap(), 1).await.unwrap();

        client.disconnect();
        assert!(matches!(client.invoke(&ping).await, Err(Error::Disconnected)));
        assert!(matches!(lease.sessions[0].invoke(&ping).await, Err(Error::Disconnected)));
        assert!(matches!(client.restart(), Err(Error::Disconnected)));
        assert!(*client.0.stop.borrow(), "a running run() wouldn't stop");
        let home = client.dc(2).unwrap();
        assert!(
            matches!(client.transfer_sessions(home, 1).await, Err(Error::Disconnected)),
            "a download could still run"
        );
    }
}
