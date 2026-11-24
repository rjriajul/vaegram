use std::sync::atomic::Ordering;

use crate::Client;

impl Client {
    /// [`disconnect`](Self::disconnect), awaiting the storage's last save instead of blocking the thread on it: on a
    /// `current_thread` runtime every other task keeps running meanwhile. When it returns, the update state is saved
    /// and the storage closed, so a new client can open the same session; an engine that takes longer than 10 seconds
    /// is given up on and reported as
    /// [`ClientEvent::StorageFailed`](crate::enums::ClientEvent::StorageFailed).
    ///
    /// Dropping the last clone saves too, but on a runtime's thread in the background (see [`Client`]): await this
    /// before opening the same session again in this process, and before `std::process::exit` or
    /// `Runtime::shutdown_background`, which don't wait for that save. Returning from `main` does.
    ///
    /// While `run` is running, the storage is saved and closed as `run` returns, as with `disconnect`; await `run`
    /// for that. Called from a handler, it doesn't wait for `run`, which waits for the handler.
    pub async fn close(&self) {
        self.stop();
        self.session().close();
        self.0.transfers.shut();
        // SeqCst on both sides, as in `disconnect`: `run` ending clears `running`, then checks this, so one of the
        // two closes the storage.
        self.0.disconnected.store(true, Ordering::SeqCst);
        if !self.0.running.load(Ordering::SeqCst) {
            self.0.close_store().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use crate::Client;
    use crate::storage::{FileStorage, Gated, Storage, UpdateState};

    #[tokio::test]
    async fn close_saves_and_lets_go_of_the_session_file() {
        let dir = std::env::temp_dir().join(format!("vaegram-close-file-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let (mut client, _sent) = Client::scripted_raw(Vec::new());
        Arc::get_mut(&mut client.0).unwrap().set_store(Arc::new(FileStorage::new(dir.join("bot"))));
        client.0.me.lock().unwrap().id = 1001;
        client.save().await.unwrap();
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 50, date: 1, ..Default::default() });

        let second = FileStorage::new(dir.join("bot"));
        let while_live = second.open().await.err().map(|e| e.kind());
        client.close().await;
        let after_close = second.open().await;
        let saved = second.load_state().await.unwrap().and_then(|s| s.update).map(|u| u.pts);
        let stopped = client.0.disconnected.load(Ordering::SeqCst) && *client.0.stop.borrow();
        drop(client);
        drop(second);
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(while_live, Some(std::io::ErrorKind::ResourceBusy), "two live clients on one file");
        assert!(after_close.is_ok(), "{after_close:?}");
        assert_eq!(saved, Some(50), "the update state is saved before close returns");
        assert!(stopped, "close didn't disconnect");
    }

    // On this `current_thread` runtime the task that lets the save through runs only if `close` yields to it.
    #[tokio::test]
    async fn close_awaits_the_save_without_blocking_the_runtime() {
        let (mut client, _sent) = Client::scripted_raw(Vec::new());
        let engine = Arc::new(Gated::default());
        Arc::get_mut(&mut client.0).unwrap().set_store(engine.clone());
        client.0.me.lock().unwrap().id = 1001;
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 50, date: 1, ..Default::default() });
        let gate = engine.clone();
        let opened = tokio::spawn(async move { gate.let_through() });
        client.close().await;
        opened.await.unwrap();
        let saved = engine.load_state().await.unwrap().and_then(|s| s.update).map(|u| u.pts);
        assert_eq!(saved, Some(50), "close returned before the save");
    }
}
