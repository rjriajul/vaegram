use crate::Client;
use crate::dispatcher::Dispatcher;
use crate::dispatcher::router::run_hook;
use crate::errors::{Error, Result};
use crate::handlers::UpdateKind;
use crate::raw::{base, functions};

impl Client {
    /// Returns `Ok(())` after `stop`, or `Error::Disconnected` when the connection ends.
    pub async fn run(&self) -> Result<()> {
        self.set_me(&self.get_me().await?);
        let saved = self.0.state.lock().unwrap().take().filter(|_| self.0.catch_up);
        let mut dispatcher = match saved {
            Some(state) => Dispatcher::resume(self.clone(), state),
            None => {
                let base::updates::State::State(state) = self.invoke(&functions::updates::GetState {}).await?;
                Dispatcher::new(self.clone(), &state)
            }
        };
        self.lifecycle_hook(UpdateKind::Start).await;
        let stopped = dispatcher.run(&mut *self.0.updates.lock().await).await;
        // Dropped first, so the update state is back in the client when the stop hook runs.
        drop(dispatcher);
        self.lifecycle_hook(UpdateKind::Stop).await;
        // Cleared here, not on entry, so a stop() that lands before run gets going still counts.
        self.0.stop.send_replace(false);
        if stopped { Ok(()) } else { Err(Error::Disconnected) }
    }

    /// Makes `run` stop taking updates, wait for the handlers already running, and return `Ok(())`.
    ///
    /// For Ctrl-C, spawn `async move { tokio::signal::ctrl_c().await.ok(); client.stop() }` with the
    /// application's own `signal`-enabled tokio.
    pub fn stop(&self) {
        self.0.stop.send_replace(true);
    }

    async fn lifecycle_hook(&self, kind: UpdateKind) {
        let hook = self.0.handlers.current().hooks.get(kind);
        if let Some(hook) = hook {
            run_hook(self.clone(), kind, hook).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::future::{Ready, ready};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use tokio::sync::{Notify, mpsc, oneshot};

    use crate::Client;
    use crate::errors::{Error, Result, RpcError};
    use crate::handlers::UpdateKind;
    use crate::raw::functions::{updates, users};
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::session::{Incoming, UpdatesRx};
    use crate::storage::state::{self, StateFile, UpdateState};

    type Log = Arc<Mutex<Vec<&'static str>>>;

    fn log_hook(log: &Log, name: &'static str) -> impl Fn(Client) -> Ready<Result<()>> + Send + Sync + 'static {
        let log = log.clone();
        move |_| {
            log.lock().unwrap().push(name);
            ready(Ok(()))
        }
    }

    // Stopped before it runs, with a saved state and an update stream that stays open.
    async fn stopped_client() -> (Client, mpsc::Sender<Incoming>) {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let empty = base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 2, seq: 3 });
        let (client, _sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(empty.to_bytes())]);
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });
        let (updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;
        client.stop();
        (client, updates)
    }

    #[tokio::test]
    async fn stop_before_run_returns_at_once_and_saves_the_state() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let empty = base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 2, seq: 3 });
        let (mut client, _sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(empty.to_bytes())]);
        let dir = std::env::temp_dir().join(format!("vaegram-stop-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bot.state");
        Arc::get_mut(&mut client.0).unwrap().state_file = Some(Arc::new(StateFile::new(path.clone())));
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });
        // Updates never arrive and the channel stays open, so only the stop can end run.
        let (_updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;

        client.stop();
        let result = client.run().await;
        let owner = client.0.me.lock().unwrap().id;
        let saved = state::load(&path, owner).map(|(_, state, _)| state.map(|s| s.pts));
        let _ = std::fs::remove_dir_all(&dir);
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(saved.ok(), Some(Some(10)));
    }

    #[tokio::test]
    async fn the_state_stays_in_memory_after_stop() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let empty = base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 2, seq: 3 });
        let (client, _sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(empty.to_bytes())]);
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });
        let (_updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;
        client.stop();
        assert!(client.run().await.is_ok());
        assert_eq!(client.0.state.lock().unwrap().as_ref().map(|s| s.pts), Some(10));
    }

    #[tokio::test]
    async fn stop_waits_for_running_handlers_then_returns_ok() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let empty = base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 2, seq: 3 });
        let (client, _sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(empty.to_bytes())]);
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });
        let (updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;

        let (started, mut running) = oneshot::channel();
        let (started, gate) = (std::sync::Mutex::new(Some(started)), Arc::new(Notify::new()));
        let release = gate.clone();
        client.on_raw_update(move |_, _| {
            let _ = started.lock().unwrap().take().map(|s| s.send(()));
            let gate = gate.clone();
            async move {
                gate.notified().await;
                Ok(())
            }
        });
        let run = tokio::spawn({
            let client = client.clone();
            async move { client.run().await }
        });
        let status = types::UpdateUserStatus { user_id: 7, status: base::UserStatus::UserStatusEmpty };
        let update = types::UpdateShort { update: base::Update::UpdateUserStatus(Box::new(status)), date: 2 };
        let updates_short = base::Updates::UpdateShort(Box::new(update));
        updates.send(Incoming { updates: updates_short, from_request: false }).await.unwrap();
        while running.try_recv().is_err() {
            tokio::task::yield_now().await;
        }

        client.stop();
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
        assert!(!run.is_finished(), "run returned before the running handler finished");
        release.notify_one();
        assert!(run.await.unwrap().is_ok());
    }

    #[tokio::test]
    async fn restart_catches_up_from_saved_state() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let empty = types::updates::DifferenceEmpty { date: 2, seq: 3 };
        let empty = base::updates::Difference::DifferenceEmpty(empty);
        let (client, mut sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(empty.to_bytes())]);
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });

        assert!(client.run().await.is_err());

        let id = |body: &[u8]| u32::from_le_bytes(body[..4].try_into().unwrap());
        assert_eq!(id(&sent.try_recv().unwrap()), users::GetUsers::CONSTRUCTOR_ID);
        let difference = sent.try_recv().unwrap();
        assert_eq!(id(&difference), updates::GetDifference::CONSTRUCTOR_ID);
        assert_eq!(difference[8..12], 10i32.to_le_bytes());
        assert!(sent.try_recv().is_err(), "a saved state must not ask for updates.getState");
    }

    #[tokio::test]
    async fn without_catch_up_run_starts_from_get_state() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let now = types::updates::State { pts: 50, qts: 0, date: 2, seq: 0, unread_count: 0 };
        let now = base::updates::State::State(Box::new(now));
        let (mut client, mut sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(now.to_bytes())]);
        Arc::get_mut(&mut client.0).unwrap().catch_up = false;
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });

        assert!(client.run().await.is_err());

        let ids: Vec<u32> = std::iter::from_fn(|| sent.try_recv().ok())
            .map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()))
            .collect();
        assert_eq!(ids, [users::GetUsers::CONSTRUCTOR_ID, updates::GetState::CONSTRUCTOR_ID]);
        assert_eq!(client.0.state.lock().unwrap().as_ref().map(|s| s.pts), Some(50));
    }

    #[tokio::test]
    async fn start_and_stop_hooks_bracket_run() {
        let (client, _updates) = stopped_client().await;
        let log = Log::default();
        client.on_stop(log_hook(&log, "stop"));
        client.on_start(log_hook(&log, "start"));
        assert!(client.run().await.is_ok());
        assert_eq!(*log.lock().unwrap(), ["start", "stop"]);
    }

    #[tokio::test]
    async fn the_stop_hook_runs_after_running_handlers() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let empty = base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 2, seq: 3 });
        let (client, _sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(empty.to_bytes())]);
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });
        let (updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;

        let (started, mut running) = oneshot::channel();
        let (started, gate) = (Mutex::new(Some(started)), Arc::new(Notify::new()));
        let (finished, release) = (Arc::new(AtomicBool::new(false)), gate.clone());
        let done = finished.clone();
        client.on_raw_update(move |_, _| {
            let _ = started.lock().unwrap().take().map(|s| s.send(()));
            let (gate, done) = (gate.clone(), done.clone());
            async move {
                gate.notified().await;
                done.store(true, Ordering::SeqCst);
                Ok(())
            }
        });
        let seen_at_stop = Arc::new(Mutex::new(None));
        let seen = seen_at_stop.clone();
        client.on_stop(move |_| {
            *seen.lock().unwrap() = Some(finished.load(Ordering::SeqCst));
            ready(Ok(()))
        });
        let run = tokio::spawn({
            let client = client.clone();
            async move { client.run().await }
        });
        let status = types::UpdateUserStatus { user_id: 7, status: base::UserStatus::UserStatusEmpty };
        let update = types::UpdateShort { update: base::Update::UpdateUserStatus(Box::new(status)), date: 2 };
        updates
            .send(Incoming { updates: base::Updates::UpdateShort(Box::new(update)), from_request: false })
            .await
            .unwrap();
        while running.try_recv().is_err() {
            tokio::task::yield_now().await;
        }
        client.stop();
        release.notify_one();
        assert!(run.await.unwrap().is_ok());
        assert_eq!(*seen_at_stop.lock().unwrap(), Some(true));
    }

    #[tokio::test]
    async fn a_failed_start_hook_reaches_on_error_and_run_goes_on() {
        let (client, _updates) = stopped_client().await;
        client.on_start(|_| ready(Err::<(), _>(Error::Unexpected("boom"))));
        let (tx, mut failed) = mpsc::unbounded_channel();
        client.on_error(move |e| {
            let _ = tx.send((e.kind, e.chat_id));
            async {}
        });
        assert!(client.run().await.is_ok());
        assert_eq!(failed.try_recv().ok(), Some((UpdateKind::Start, None)));
    }

    #[tokio::test]
    async fn no_hooks_when_run_fails_before_starting() {
        let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::new(401, "AUTH_KEY_UNREGISTERED")))]);
        let log = Log::default();
        client.on_start(log_hook(&log, "start"));
        client.on_stop(log_hook(&log, "stop"));
        assert!(client.run().await.is_err());
        assert!(log.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn registering_again_replaces_the_hook() {
        let (client, _updates) = stopped_client().await;
        let log = Log::default();
        let first = client.on_start(log_hook(&log, "first")).id();
        client.on_start(log_hook(&log, "second"));
        assert!(!client.remove_handler(first), "a replaced hook is already gone");
        assert!(client.run().await.is_ok());
        assert_eq!(*log.lock().unwrap(), ["second"]);
    }

    #[tokio::test]
    async fn a_removed_start_hook_does_not_run() {
        let (client, _updates) = stopped_client().await;
        let log = Log::default();
        let start = client.on_start(log_hook(&log, "start")).id();
        client.on_stop(log_hook(&log, "stop"));
        assert!(client.remove_handler(start));
        assert!(client.run().await.is_ok());
        assert_eq!(*log.lock().unwrap(), ["stop"]);
    }
}
