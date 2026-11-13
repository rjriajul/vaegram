use std::sync::atomic::Ordering;

use crate::Client;
use crate::dispatcher::Dispatcher;
use crate::dispatcher::router::run_hook;
use crate::errors::{Error, Result};
use crate::handlers::UpdateKind;
use crate::raw::{base, functions};
use crate::session::internals::now;

// The server's clock is read in whole seconds, so the run's start, in Telegram's time, may read up to a second
// late.
const CLOCK_SLACK: i64 = 1;

/// The run's start, in Telegram's time, as the dispatcher holds it: clamped, not wrapped, when the server's clock
/// reads past what an `i32` holds.
fn start_time(started: i64) -> i32 {
    started.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

impl Client {
    /// Takes updates and runs the handlers until `stop`, then returns `Ok(())`; it returns `Error::Disconnected`
    /// when the connection ends. Fails at once with `Error::InvalidArgument` while this client is already running,
    /// here or in [`compose`](Client::compose).
    ///
    /// Telegram sends updates to every connected session of an account, bots included, so several clients logged
    /// in as the same bot or user, in one process or several, each get every update and each run their handlers:
    /// a handler that replies then replies once per client. With [`catch_up`](crate::ClientBuilder::catch_up) on,
    /// the default, `run` first replays what came since the saved update state, so a client started from an old
    /// session, or from a copy of another client's, handles those updates again too.
    pub async fn run(&self) -> Result<()> {
        let running = self.claim_run()?;
        match self.prepare().await? {
            Some(dispatcher) => self.drive(running, dispatcher).await,
            None => Ok(()),
        }
    }

    /// The client's updates for one `run`, until the returned guard is dropped.
    pub(super) fn claim_run(&self) -> Result<Running> {
        if self.0.running.swap(true, Ordering::AcqRel) {
            return Err(Error::InvalidArgument("this client is already running"));
        }
        Ok(Running(self.clone()))
    }

    /// What anything that waits for updates checks first: a client built with `no_updates` never gets any.
    pub(crate) fn require_updates(&self) -> Result<()> {
        if self.0.options.no_updates {
            return Err(Error::InvalidArgument("updates are off for this client"));
        }
        Ok(())
    }

    /// Everything `run` does before it takes updates: who the account is, where the update state starts, and the
    /// start hook. `None` when `disconnect` cut it short: that stops the client before it ends the connection, so a
    /// start request failing with `Disconnected` then ends a stopped run, not a lost one. Untagged, like all of
    /// `run`: what it sends is vaegram's own doing, even inside a caller's tag.
    pub(super) async fn prepare(&self) -> Result<Option<Dispatcher>> {
        match crate::observe::untagged(self.start_dispatcher()).await {
            Err(Error::Disconnected) if *self.0.stop.borrow() => {
                // Cleared as `drive` clears it when a run ends.
                self.0.stop.send_replace(false);
                Ok(None)
            }
            started => started.map(Some),
        }
    }

    async fn start_dispatcher(&self) -> Result<Dispatcher> {
        self.require_updates()?;
        let local_start = now();
        self.set_me(&self.get_me_raw().await?);
        // When the run began, in Telegram's time, as the getMe reply just showed its clock: without catch-up, a bot
        // drops what Telegram dated before it.
        let started = local_start + self.session().seen_offset() - CLOCK_SLACK;
        let saved = self.0.state.lock().unwrap().take().filter(|_| self.0.catch_up);
        let dispatcher = match saved {
            Some(state) => Dispatcher::resume(self.clone(), state),
            None => {
                let base::updates::State::State(state) = self.invoke(&functions::updates::GetState {}).await?;
                if self.0.catch_up || !self.0.me.lock().unwrap().is_bot {
                    // With catch-up on and nothing saved, what Telegram still holds is delivered as it comes. A
                    // user's getState is current, so there is nothing from before to skip.
                    Dispatcher::new(self.clone(), &state)
                } else {
                    Dispatcher::from_now(self.clone(), &state, start_time(started))
                }
            }
        };
        self.lifecycle_hook(UpdateKind::Start).await;
        Ok(dispatcher)
    }

    /// Takes updates until `stop` or a lost connection, then runs the stop hook; untagged, as [`prepare`] is.
    ///
    /// [`prepare`]: Self::prepare
    // `dispatcher` comes after `running`, so a cancelled `drive` drops it first: it hands its state back to the
    // client before `Running` saves it for a disconnected client.
    pub(super) async fn drive(&self, running: Running, mut dispatcher: Dispatcher) -> Result<()> {
        let stopped = crate::observe::untagged(dispatcher.run(&mut *self.0.updates.lock().await)).await;
        // Dropped first, so the update state is back in the client when the stop hook runs.
        drop(dispatcher);
        crate::observe::untagged(self.lifecycle_hook(UpdateKind::Stop)).await;
        // Cleared here, not on entry, so a stop() that lands before run gets going still counts.
        self.0.stop.send_replace(false);
        drop(running);
        if stopped { Ok(()) } else { Err(Error::Disconnected) }
    }

    /// Makes `run` stop taking updates, wait for the handlers already running, and return `Ok(())`.
    ///
    /// For Ctrl-C, use [`run_until_ctrl_c`](Self::run_until_ctrl_c) in place of `run`; for Ctrl-C or SIGTERM, as a
    /// service needs, [`run_until_signal`](Self::run_until_signal).
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

/// Held while a `run` owns the client's updates; dropping it, however `run` ends, lets the next one start, and
/// closes the storage of a client `disconnect` was called on meanwhile.
pub(super) struct Running(Client);

impl Drop for Running {
    fn drop(&mut self) {
        let inner = &self.0.0;
        inner.running.store(false, Ordering::SeqCst);
        if inner.disconnected.load(Ordering::SeqCst) {
            inner.finish_store();
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use std::future::{Ready, ready};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use tokio::sync::{Notify, mpsc, oneshot};

    use crate::Client;
    use crate::client::Me;
    use crate::errors::{Error, Result, RpcError};
    use crate::handlers::UpdateKind;
    use crate::raw::functions::{updates, users};
    use crate::raw::{Identifiable, Serializable, base, types};
    use crate::session::{Incoming, UpdatesRx};
    use crate::storage::{FileStorage, MemoryStorage, Storage, UpdateState};
    use crate::utils::get_peer;

    type Log = Arc<Mutex<Vec<&'static str>>>;

    fn log_hook(log: &Log, name: &'static str) -> impl Fn(Client) -> Ready<Result<()>> + Send + Sync + 'static {
        let log = log.clone();
        move |_| {
            log.lock().unwrap().push(name);
            ready(Ok(()))
        }
    }

    // Stopped before it runs, with a saved state and an update stream that stays open.
    pub(in crate::methods::utilities) async fn stopped_client() -> (Client, mpsc::Sender<Incoming>) {
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
    async fn run_refuses_without_updates() {
        let (mut client, mut sent) = Client::recording();
        client.set_options(crate::connection::Options::new().no_updates(true));
        let refused = client.run().await;
        assert!(matches!(refused, Err(Error::InvalidArgument(_))), "{refused:?}");
        let listened = client.listen::<crate::types::Message>().await;
        assert!(matches!(listened, Err(Error::InvalidArgument(_))), "{listened:?}");
        let asked = client.ask("me", "hi").await;
        assert!(matches!(asked, Err(Error::InvalidArgument(_))), "{asked:?}");
        assert!(sent.try_recv().is_err(), "something went out");
    }

    #[tokio::test]
    async fn stop_before_run_returns_at_once_and_saves_the_state() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let empty = base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 2, seq: 3 });
        let (mut client, _sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(empty.to_bytes())]);
        let engine = Arc::new(MemoryStorage::new());
        Arc::get_mut(&mut client.0).unwrap().set_store(engine.clone());
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });
        // Updates never arrive and the channel stays open, so only the stop can end run.
        let (_updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;

        client.stop();
        let result = client.run().await;
        let owner = client.0.me.lock().unwrap().id;
        let saved = engine.load_state().await.unwrap().map(|s| (s.owner, s.update.map(|u| u.pts)));
        assert!(result.is_ok(), "{result:?}");
        assert_eq!(saved, Some((owner, Some(10))));
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
    async fn a_second_run_while_one_is_running_is_refused() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let empty = base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 2, seq: 3 });
        let (client, mut sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(empty.to_bytes())]);
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });
        let (_updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;
        let starts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = starts.clone();
        client.on_start(move |_| {
            counted.fetch_add(1, Ordering::Relaxed);
            ready(Ok(()))
        });
        let first = tokio::spawn({
            let client = client.clone();
            async move { client.run().await }
        });
        // users.getUsers, then updates.getDifference: the first run is taking updates.
        sent.recv().await;
        sent.recv().await;

        let second = client.run().await;
        assert!(matches!(second, Err(Error::InvalidArgument(_))), "{second:?}");
        assert!(sent.try_recv().is_err(), "the second run sent something");
        client.stop();
        assert!(first.await.unwrap().is_ok());
        assert_eq!(starts.load(Ordering::Relaxed), 1);
        assert!(!client.0.running.load(Ordering::Relaxed), "a later run would be refused too");
    }

    // The test kit's way to end: a handler holds a clone of its client, `disconnect`, then the runtime goes and
    // cancels `run`.
    #[tokio::test]
    async fn a_disconnected_run_saves_and_lets_go_of_the_session_file_as_it_ends() {
        let dir = std::env::temp_dir().join(format!("vaegram-run-disconnect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let empty = base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 2, seq: 3 });
        let (mut client, mut sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(empty.to_bytes())]);
        Arc::get_mut(&mut client.0).unwrap().set_store(Arc::new(FileStorage::new(dir.join("bot"))));
        client.save().await.unwrap();
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });
        let (_updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;
        let own = client.clone();
        client.on_raw_update(move |_, _| {
            let _own = &own;
            ready(Ok(()))
        });
        let run = tokio::spawn({
            let client = client.clone();
            async move { client.run().await }
        });
        // users.getUsers, then updates.getDifference: the run is taking updates.
        sent.recv().await;
        sent.recv().await;

        client.disconnect();
        run.abort();
        let _ = run.await;
        let second = FileStorage::new(dir.join("bot"));
        let reopened = second.open().await;
        let saved = second.load_state().await.ok().flatten().and_then(|s| s.update).map(|u| (u.pts, u.date));
        drop(second);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(reopened.is_ok(), "{reopened:?}");
        assert_eq!(saved, Some((10, 2)), "the run's state is saved before the file is let go");
    }

    #[tokio::test]
    async fn run_inside_tagged_sends_nothing_tagged() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let empty = base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 2, seq: 3 });
        let pong = || Ok(base::Pong::Pong(Box::new(types::Pong { msg_id: 0, ping_id: 1 })).to_bytes());
        let replies = vec![Ok(me.to_bytes()), pong(), Ok(empty.to_bytes()), pong()];
        let (client, mut sent) = Client::scripted_tags(replies);
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });
        let ping = |client: Client| async move { client.invoke(&crate::raw::functions::Ping { ping_id: 1 }).await };
        client.on_start(ping);
        client.on_stop(ping);

        assert!(crate::observe::tagged(7, client.run()).await.is_err());
        let id = |body: &[u8]| u32::from_le_bytes(body[..4].try_into().unwrap());
        let sent: Vec<_> = std::iter::from_fn(|| sent.try_recv().ok()).map(|(body, tag)| (id(&body), tag)).collect();
        let ping = <crate::raw::functions::Ping as Identifiable>::CONSTRUCTOR_ID;
        let expected = [users::GetUsers::CONSTRUCTOR_ID, ping, updates::GetDifference::CONSTRUCTOR_ID, ping];
        assert_eq!(sent, expected.map(|id| (id, None)));
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
    async fn without_catch_up_a_bot_starts_from_get_state() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let now = types::updates::State { pts: 50, qts: 0, date: 2, seq: 0, unread_count: 0 };
        let now = base::updates::State::State(Box::new(now));
        let nothing_pending =
            base::updates::Difference::DifferenceEmpty(types::updates::DifferenceEmpty { date: 3, seq: 0 });
        let replies = vec![Ok(me.to_bytes()), Ok(now.to_bytes()), Ok(nothing_pending.to_bytes())];
        let (mut client, mut sent) = Client::scripted(replies);
        Arc::get_mut(&mut client.0).unwrap().catch_up = false;
        *client.0.state.lock().unwrap() = Some(UpdateState { pts: 10, date: 1, ..Default::default() });

        assert!(client.run().await.is_err());

        let bodies: Vec<Vec<u8>> = std::iter::from_fn(|| sent.try_recv().ok()).collect();
        let ids: Vec<u32> = bodies.iter().map(|b| u32::from_le_bytes(b[..4].try_into().unwrap())).collect();
        let expected = [
            users::GetUsers::CONSTRUCTOR_ID,
            updates::GetState::CONSTRUCTOR_ID,
            updates::GetDifference::CONSTRUCTOR_ID,
        ];
        assert_eq!(ids, expected);
        // The difference starts from getState's answer, not the saved state.
        assert_eq!(bodies[2][8..12], 50i32.to_le_bytes());
        assert_eq!(client.0.state.lock().unwrap().as_ref().map(|s| (s.pts, s.date)), Some((50, 3)));
    }

    // A user's getState is current, so a user's run starts from it as it is.
    #[tokio::test]
    async fn without_catch_up_a_user_starts_from_get_state_without_a_difference() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let now = types::updates::State { pts: 50, qts: 0, date: 2, seq: 0, unread_count: 0 };
        let (mut client, mut sent) =
            Client::scripted(vec![Ok(me.to_bytes()), Ok(base::updates::State::State(Box::new(now)).to_bytes())]);
        Arc::get_mut(&mut client.0).unwrap().catch_up = false;
        // Test clients are bots; this one is a user.
        *client.0.me.lock().unwrap() = Me { id: 1, is_bot: false, ..Me::default() };

        assert!(client.run().await.is_err());

        let ids: Vec<u32> = std::iter::from_fn(|| sent.try_recv().ok())
            .map(|b| u32::from_le_bytes(b[..4].try_into().unwrap()))
            .collect();
        assert_eq!(ids, [users::GetUsers::CONSTRUCTOR_ID, updates::GetState::CONSTRUCTOR_ID]);
        assert_eq!(client.0.state.lock().unwrap().as_ref().map(|s| (s.pts, s.date)), Some((50, 2)));
    }

    fn connected(id: &str, qts: i32) -> base::Update {
        let connection = types::BotBusinessConnection {
            disabled: false,
            connection_id: id.into(),
            user_id: 7,
            dc_id: 2,
            date: 1,
            rights: None,
        };
        let connection = base::BotBusinessConnection::BotBusinessConnection(Box::new(connection));
        base::Update::UpdateBotBusinessConnect(Box::new(types::UpdateBotBusinessConnect { connection, qts }))
    }

    // A bot's updates.getState names the last update Telegram knows it confirmed, so updates it pushed in an earlier
    // run come again after it. Without catch-up they are skipped: the difference since then is read, and what it
    // brings from before the run is dropped.
    #[tokio::test]
    async fn without_catch_up_updates_from_before_are_skipped() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let confirmed = types::updates::State { pts: 50, qts: 5, date: 2, seq: 0, unread_count: 0 };
        let now = types::updates::State { qts: 6, date: 3, ..confirmed.clone() };
        let pending = types::updates::Difference {
            new_messages: Vec::new(),
            new_encrypted_messages: Vec::new(),
            other_updates: vec![connected("old", 6)],
            chats: Vec::new(),
            users: Vec::new(),
            state: base::updates::State::State(Box::new(now)),
        };
        let pending = base::updates::Difference::Difference(Box::new(pending));
        let confirmed = base::updates::State::State(Box::new(confirmed));
        let replies = vec![Ok(me.to_bytes()), Ok(confirmed.to_bytes()), Ok(pending.to_bytes())];
        let (mut client, _sent) = Client::scripted(replies);
        Arc::get_mut(&mut client.0).unwrap().catch_up = false;
        let (updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        client.on_business_connection(crate::filters::all(), move |c| {
            log.lock().unwrap().push(c.id);
            ready(Ok(()))
        });
        // Telegram pushes the old one again, then a new one.
        for (id, qts) in [("old", 6), ("new", 7)] {
            let short = types::UpdateShort { update: connected(id, qts), date: 3 };
            let incoming = Incoming { updates: base::Updates::UpdateShort(Box::new(short)), from_request: false };
            updates.send(incoming).await.unwrap();
        }
        let run = tokio::spawn({
            let client = client.clone();
            async move { client.run().await }
        });
        while seen.lock().unwrap().is_empty() {
            tokio::task::yield_now().await;
        }
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
        client.stop();
        assert!(run.await.unwrap().is_ok());
        assert_eq!(*seen.lock().unwrap(), ["new"]);
    }

    fn new_message(id: i32, date: i32, pts: i32) -> base::Update {
        let message = base::Message::Message(Box::new(crate::types::Message::bare(id, get_peer(7), date)));
        base::Update::UpdateNewMessage(Box::new(types::UpdateNewMessage { message, pts, pts_count: 1 }))
    }

    /// The ids of the new messages a run without catch-up hands to its handlers, waiting until `expected` came,
    /// when getState says pts 50, the difference after it brings `held` and ends at pts `to`, `pushed` waits on
    /// the update stream, and the server's clock runs `server_ahead` seconds ahead of this one.
    async fn delivered_from_now(
        held: Vec<base::Update>,
        to: i32,
        pushed: Vec<base::Update>,
        expected: usize,
        server_ahead: i64,
    ) -> Vec<i32> {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let state = |pts| types::updates::State { pts, qts: 0, date: 2, seq: 0, unread_count: 0 };
        let held = held.into_iter().map(|u| match u {
            base::Update::UpdateNewMessage(u) => u.message,
            _ => unreachable!("only new messages are held here"),
        });
        let pending = types::updates::Difference {
            new_messages: held.collect(),
            new_encrypted_messages: Vec::new(),
            other_updates: Vec::new(),
            chats: Vec::new(),
            users: Vec::new(),
            state: base::updates::State::State(Box::new(state(to))),
        };
        let replies = vec![
            Ok(me.to_bytes()),
            Ok(base::updates::State::State(Box::new(state(50))).to_bytes()),
            Ok(base::updates::Difference::Difference(Box::new(pending)).to_bytes()),
        ];
        let (mut client, _sent) = Client::scripted(replies);
        Arc::get_mut(&mut client.0).unwrap().catch_up = false;
        client.session().set_seen_offset(server_ahead);
        let (updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        client.on_raw_update(move |_, raw| {
            if let base::Update::UpdateNewMessage(u) = &raw.update {
                if let base::Message::Message(m) = &u.message {
                    log.lock().unwrap().push(m.id);
                }
            }
            ready(Ok(()))
        });
        for update in pushed {
            let batch = types::Updates { updates: vec![update], users: Vec::new(), chats: Vec::new(), date: 0, seq: 0 };
            let incoming = Incoming { updates: base::Updates::Updates(Box::new(batch)), from_request: false };
            updates.send(incoming).await.unwrap();
        }
        let run = tokio::spawn({
            let client = client.clone();
            async move { client.run().await }
        });
        for _ in 0..1000 {
            if seen.lock().unwrap().len() >= expected {
                break;
            }
            tokio::task::yield_now().await;
        }
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
        client.stop();
        assert!(run.await.unwrap().is_ok());
        std::mem::take(&mut *seen.lock().unwrap())
    }

    fn now() -> i32 {
        crate::session::internals::now() as i32
    }

    // The test kit's case: the user's message reaches Telegram while the bot's run reads what Telegram held.
    #[tokio::test]
    async fn without_catch_up_what_came_since_the_run_began_is_delivered() {
        let held = vec![new_message(9, 1, 51), new_message(1, now(), 52)];
        assert_eq!(delivered_from_now(held, 52, Vec::new(), 1, 0).await, [1]);
    }

    // Telegram pushes a message it also puts in the difference, then one after it.
    #[tokio::test]
    async fn without_catch_up_a_message_both_held_and_pushed_runs_once() {
        let pushed = vec![new_message(1, now(), 51), new_message(2, now(), 52)];
        assert_eq!(delivered_from_now(vec![new_message(1, now(), 51)], 51, pushed, 2, 0).await, [1, 2]);
    }

    // The run's start is read off the clock the server's messages show, not the offset saved with the salt: here
    // the server runs an hour ahead, so a message dated by this clock's now came before the run began.
    #[tokio::test]
    async fn without_catch_up_the_run_begins_by_the_server_clock() {
        let held = vec![new_message(1, now(), 51), new_message(2, now() + 3600, 52)];
        assert_eq!(delivered_from_now(held, 52, Vec::new(), 1, 3600).await, [2]);
    }

    // A server clock read before what an `i32` holds clamps, so the dispatcher never wraps the run's start.
    #[test]
    fn a_start_time_before_i32_min_clamps() {
        assert_eq!(super::start_time(i64::from(i32::MIN) - 1), i32::MIN);
    }

    // A clock it reads past the end of an `i32` clamps too, rather than dropping every update as held.
    #[test]
    fn a_start_time_after_i32_max_clamps() {
        assert_eq!(super::start_time(i64::from(i32::MAX) + 1), i32::MAX);
    }

    // Telegram pushes what it held again, and reading it fails at first: until a difference is read, only what
    // came since the run began is delivered from the stream too.
    #[tokio::test]
    async fn without_catch_up_a_held_update_pushed_before_the_first_difference_is_skipped() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let confirmed = types::updates::State { pts: 50, qts: 5, date: 2, seq: 0, unread_count: 0 };
        let confirmed = base::updates::State::State(Box::new(confirmed));
        let flood = Err(Error::Rpc(RpcError::parse(420, "FLOOD_WAIT_1")));
        let (mut client, _sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(confirmed.to_bytes()), flood]);
        Arc::get_mut(&mut client.0).unwrap().catch_up = false;
        let (updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        client.on_raw_update(move |_, raw| {
            log.lock().unwrap().push(match &raw.update {
                base::Update::UpdateBotBusinessConnect(_) => "old connection",
                base::Update::UpdateNewMessage(_) => "new message",
                base::Update::UpdateBotCallbackQuery(_) => "callback query",
                _ => "other",
            });
            ready(Ok(()))
        });
        let query = types::UpdateBotCallbackQuery {
            query_id: 1,
            user_id: 7,
            peer: get_peer(7),
            msg_id: 1,
            chat_instance: 3,
            data: Some(b"tap".to_vec()),
            game_short_name: None,
        };
        // A callback query moves no counter, so Telegram never holds one: it passes.
        let query = base::Update::UpdateBotCallbackQuery(Box::new(query));
        for update in [connected("old", 6), new_message(1, now(), 51), query] {
            let short = types::UpdateShort { update, date: 3 };
            let incoming = Incoming { updates: base::Updates::UpdateShort(Box::new(short)), from_request: false };
            updates.send(incoming).await.unwrap();
        }
        let run = tokio::spawn({
            let client = client.clone();
            async move { client.run().await }
        });
        for _ in 0..1000 {
            if seen.lock().unwrap().len() >= 2 {
                break;
            }
            tokio::task::yield_now().await;
        }
        for _ in 0..50 {
            tokio::task::yield_now().await;
        }
        client.stop();
        assert!(run.await.unwrap().is_ok());
        assert_eq!(*seen.lock().unwrap(), ["new message", "callback query"]);
    }

    // Telegram refuses the first difference, so what it held won't be read: from then on nothing more is taken for
    // it, and an undated update pushed after reaches the handlers.
    #[tokio::test]
    async fn without_catch_up_a_refused_first_difference_ends_the_skipping() {
        let me = vec![base::User::UserEmpty(types::UserEmpty { id: 1 })];
        let confirmed = types::updates::State { pts: 50, qts: 0, date: 2, seq: 0, unread_count: 0 };
        let confirmed = base::updates::State::State(Box::new(confirmed));
        let refused = Err(Error::Rpc(RpcError::parse(400, "X_INVALID")));
        let (mut client, _sent) = Client::scripted(vec![Ok(me.to_bytes()), Ok(confirmed.to_bytes()), refused]);
        Arc::get_mut(&mut client.0).unwrap().catch_up = false;
        let (updates, rx) = UpdatesRx::channel();
        *client.0.updates.lock().await = rx;
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        client.on_raw_update(move |_, raw| {
            if let base::Update::UpdateDeleteMessages(u) = &raw.update {
                log.lock().unwrap().push(u.pts);
            }
            ready(Ok(()))
        });
        // The run reads the stream only after its first difference was refused.
        let deleted = types::UpdateDeleteMessages { messages: vec![1], pts: 51, pts_count: 1 };
        let short = types::UpdateShort { update: base::Update::UpdateDeleteMessages(Box::new(deleted)), date: 3 };
        let incoming = Incoming { updates: base::Updates::UpdateShort(Box::new(short)), from_request: false };
        updates.send(incoming).await.unwrap();
        let run = tokio::spawn({
            let client = client.clone();
            async move { client.run().await }
        });
        for _ in 0..1000 {
            if !seen.lock().unwrap().is_empty() {
                break;
            }
            tokio::task::yield_now().await;
        }
        client.stop();
        assert!(run.await.unwrap().is_ok());
        assert_eq!(*seen.lock().unwrap(), [51]);
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
        let (client, _sent) = Client::scripted(vec![Err(Error::Rpc(RpcError::parse(401, "AUTH_KEY_UNREGISTERED")))]);
        let log = Log::default();
        client.on_start(log_hook(&log, "start"));
        client.on_stop(log_hook(&log, "stop"));
        assert!(client.run().await.is_err());
        assert!(log.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_disconnect_during_the_start_ends_run_with_ok() {
        // `disconnect` stops the client, then fails the start's waiting request with `Disconnected`.
        let (client, _sent) = Client::scripted(vec![Err(Error::Disconnected)]);
        let log = Log::default();
        client.on_start(log_hook(&log, "start"));
        client.on_stop(log_hook(&log, "stop"));
        client.disconnect();
        let result = client.run().await;
        assert!(result.is_ok(), "{result:?}");
        assert!(log.lock().unwrap().is_empty(), "the start never finished, so neither hook runs");
        assert!(!*client.0.stop.borrow(), "a finished run clears the stop");

        // Without a stop, a connection that ends during the start is still an error.
        let (client, _sent) = Client::scripted(vec![Err(Error::Disconnected)]);
        assert!(matches!(client.run().await, Err(Error::Disconnected)));
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
