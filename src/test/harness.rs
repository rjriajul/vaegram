//! Updates for a test client: fed straight to its handlers, or pushed onto the stream its `run` reads.

use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError, Weak};
use std::time::Duration;

use tokio::task::JoinSet;
use tokio::time::Instant;

use super::updates::{TestChat, TestUpdate, TestUser};
use super::{Recorder, Script, TestError};
use crate::Client;
use crate::client::Inner;
use crate::dispatcher::{Entities, Handling};
use crate::errors::Result;
use crate::raw::base;
use crate::session::Incoming;

/// How often [`Recorder::push_stream`] looks again for a `run` that is taking updates.
const RUN_POLL: Duration = Duration::from_millis(1);
/// How long [`Recorder::push_stream`] waits for a `run` to start taking updates.
const RUN_START: Duration = Duration::from_secs(2);

/// Handler tasks taken out of the client's [`Handling`] to wait for; dropped unfinished, as when `settle` is
/// cancelled, they go on running instead of being aborted.
struct Detached(JoinSet<()>);

impl Drop for Detached {
    fn drop(&mut self) {
        self.0.detach_all();
    }
}

/// Feeding updates to the handlers registered on the client, as Telegram would send them.
///
/// [`feed`](Self::feed) and [`dispatch`](Self::dispatch) hand an update to the client's handlers, filters,
/// listeners and `on_error` with no `run` going, through the same routes and handler tasks as `run`, `ordered` and
/// `max_concurrent_handlers` included; the users and chats the update names land in the client's peer cache, so
/// `client.send_message(user_id, ..)` works too. They skip what `run` does before the handlers: holding updates
/// for a `pts` gap, `getDifference`, and dropping the updates that came back with a request. To test those, push
/// updates with [`push_stream`](Self::push_stream) to a client whose `run` takes them.
///
/// A handler that waits for a second update, in [`Client::ask`] or [`Client::listen`], keeps `feed` waiting
/// until that update comes, so its first update goes in with `dispatch`, the question is awaited with
/// [`recv`](Self::recv), and the answer is fed, then [`settle`](Self::settle) waits for what's left:
///
/// ```no_run
/// use vaegram::Client;
/// use vaegram::filters;
/// use vaegram::test::TestMessage;
///
/// # async fn f() {
/// let (client, recorder) = Client::recording();
/// client.on_message(filters::command("name"), |msg| async move {
///     let answer = msg.client().ask(msg.chat.id, "What's your name?").await?;
///     msg.reply(format!("Hi, {}", answer.text())).await
/// });
///
/// recorder.dispatch(TestMessage::private(42, "/name")).await;
/// recorder.recv().await; // the question
/// recorder.feed(TestMessage::private(42, "Ann")).await;
/// recorder.settle().await;
/// assert_eq!(recorder.sent_texts(42), ["What's your name?", "Hi, Ann"]);
/// # }
/// ```
impl Recorder {
    /// Makes the client know `user`, so it can send to them before any update named them.
    pub fn know_user(&self, user: TestUser) -> &Self {
        if let Some(client) = self.client() {
            let _ = Entities::new(&client, vec![user.raw()], Vec::new());
        }
        self
    }

    /// Makes the client know `chat`, so it can send there before any update named it.
    pub fn know_chat(&self, chat: TestChat) -> &Self {
        if let Some(client) = self.client() {
            let _ = match chat.raw() {
                Some(raw) => Entities::new(&client, Vec::new(), vec![raw]),
                None => Entities::new(&client, vec![TestUser::new(chat.id()).raw()], Vec::new()),
            };
        }
        self
    }

    /// Hands `update` to the client's handlers and waits, as [`settle`](Self::settle) does, for every handler
    /// running to finish, those that earlier [`dispatch`](Self::dispatch) calls started included.
    pub async fn feed(&self, update: impl Into<TestUpdate>) {
        self.dispatch(update).await;
        self.settle().await;
    }

    /// Hands `update` to the client's handlers and returns once they are started, not finished, for a handler
    /// that waits for another update; then [`settle`](Self::settle).
    pub async fn dispatch(&self, update: impl Into<TestUpdate>) {
        let Some(client) = self.client() else { return };
        // Fed past the tracker, so the `pts` stays: a `run` going meanwhile sees no gap.
        let updates = update.into().into_raw(&self.0, false);
        let mut handling = self.0.handling.lock().unwrap_or_else(PoisonError::into_inner);
        route(&client, handling.get_or_insert_with(|| Handling::new(&client)), updates);
    }

    /// Waits until no handler [`feed`](Self::feed) and [`dispatch`](Self::dispatch) started is running, those
    /// started meanwhile included. Updates can be fed from another task while it waits.
    pub async fn settle(&self) {
        let Some(client) = self.client() else { return };
        loop {
            let running = {
                let mut handling = self.0.handling.lock().unwrap_or_else(PoisonError::into_inner);
                let Some(handling) = handling.as_mut() else { return };
                // What listeners gave back goes to the handlers again, as `run` does.
                handling.redeliver(&client);
                handling.tasks.take_running()
            };
            if running.is_empty() {
                return;
            }
            let mut running = Detached(running);
            while running.0.join_next().await.is_some() {}
        }
    }

    /// Pushes `update` onto the stream the client's [`run`](Client::run) reads, as Telegram pushes it: through
    /// the tracker, which holds it on a `pts` gap. Start `run` on a task of its own first: this waits, up to 2
    /// seconds, until `run` has the update state, so the update's `pts` comes right after it, then while 8 pushed
    /// updates are still unread.
    ///
    /// Fails with [`TestError::NoRun`] when no `run` takes it: none started in time, or it failed to start (a
    /// strict [`Script`] must answer `users.getUsers` and `updates.getState`), or it ended, or the stream was
    /// closed.
    pub async fn push_stream(&self, update: impl Into<TestUpdate>) -> Result<()> {
        let stream = self.0.stream.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let stream = stream.ok_or(TestError::NoRun)?;
        let start = Instant::now();
        let mut started = false;
        loop {
            let client = self.client().ok_or(TestError::NoRun)?;
            if client.0.recover.lock().unwrap_or_else(PoisonError::into_inner).is_some() {
                break;
            }
            let running = client.0.running.load(Ordering::Acquire);
            // A `run` seen starting that stopped before it took updates failed to start.
            if (started && !running) || start.elapsed() >= RUN_START {
                return Err(TestError::NoRun.into());
            }
            started |= running;
            drop(client);
            tokio::time::sleep(RUN_POLL).await;
        }
        // The slot first: a push cancelled while the queue is full moves no `pts`.
        let slot = stream.reserve().await.map_err(|_| TestError::NoRun)?;
        slot.send(Incoming { updates: update.into().into_raw(&self.0, true), from_request: false });
        Ok(())
    }

    /// Ends the stream [`push_stream`](Self::push_stream) fills: `run` returns `Err(Error::Disconnected)` once
    /// it read what was pushed, as when the connection is lost. Later pushes fail.
    pub fn close_stream(&self) {
        self.0.stream.lock().unwrap_or_else(PoisonError::into_inner).take();
    }

    fn client(&self) -> Option<Client> {
        self.0.client.get().and_then(Weak::upgrade).map(|inner: Arc<Inner>| Client(inner, None))
    }
}

/// Hands `updates` to the handlers and listeners as `run` would once its tracker released them.
fn route(client: &Client, handling: &mut Handling, updates: base::Updates) {
    // A `run` that ended closed the listeners; with none going, `ask` and `listen` work for fed updates.
    if !client.0.running.load(Ordering::Acquire) {
        client.0.handlers.listeners.open();
    }
    handling.redeliver(client);
    let (updates, entities) = match updates {
        base::Updates::Updates(u) => (u.updates, Entities::new(client, u.users, u.chats)),
        base::Updates::UpdatesCombined(u) => (u.updates, Entities::new(client, u.users, u.chats)),
        base::Updates::UpdateShort(u) => (vec![u.update], Entities::default()),
        // The short message forms and `UpdatesTooLong` only make sense to the tracker: `push_stream` takes them.
        _ => return,
    };
    let entities = Arc::new(entities);
    for update in updates {
        handling.deliver(client, update, &entities);
    }
}

impl Script {
    /// As [`Recorder::know_user`].
    pub fn know_user(&self, user: TestUser) -> &Self {
        self.0.know_user(user);
        self
    }

    /// As [`Recorder::know_chat`].
    pub fn know_chat(&self, chat: TestChat) -> &Self {
        self.0.know_chat(chat);
        self
    }

    /// As [`Recorder::feed`].
    pub async fn feed(&self, update: impl Into<TestUpdate>) {
        self.0.feed(update).await;
    }

    /// As [`Recorder::dispatch`].
    pub async fn dispatch(&self, update: impl Into<TestUpdate>) {
        self.0.dispatch(update).await;
    }

    /// As [`Recorder::settle`].
    pub async fn settle(&self) {
        self.0.settle().await;
    }

    /// As [`Recorder::push_stream`].
    pub async fn push_stream(&self, update: impl Into<TestUpdate>) -> Result<()> {
        self.0.push_stream(update).await
    }

    /// As [`Recorder::close_stream`].
    pub fn close_stream(&self) {
        self.0.close_stream();
    }
}
