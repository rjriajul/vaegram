use std::any::Any;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::task::Poll;
use std::time::Duration;

use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot};
use tokio::task::JoinSet;
use tokio::time::Instant;

use crate::Client;
use crate::enums::ClientEvent;
use crate::errors::Error;
use crate::handlers::{BoxFuture, Callback, ErrorCallback, HandlerError, Routed, UpdateKind};
use crate::session::Events;

/// With `max_concurrent_handlers`, how many handler tasks may wait for a slot before the dispatcher stops taking
/// updates. Past it, updates wait in the update queue, whose overflow getDifference fetches again, instead of
/// piling up in memory. With `ordered`, a task waiting behind its own chat's isn't counted there, so one busy chat
/// can't hold up the others; as many may wait in each chat, and the chat's next update past them is dropped.
pub(crate) const MAX_WAITING_HANDLERS: usize = 1024;

/// With `ordered` and `max_concurrent_handlers`, how many tasks may wait behind their own chat's over every chat;
/// past it, the next update that would wait so is dropped, as past one chat's bound, and intake goes on. Four
/// chats' worth, so one busy chat never reaches it alone, while memory stays bounded however many chats flood.
const MAX_WAITING_IN_LANES: usize = 4 * MAX_WAITING_HANDLERS;

#[derive(Default)]
pub(crate) struct Tasks {
    lanes: Option<Lanes>,
    permits: Option<Arc<Semaphore>>,
    // Started and unfinished tasks past which `full` says so; only with a handler cap.
    limit: Option<usize>,
    pub(crate) on_error: Option<ErrorCallback>,
    pub(crate) events: Arc<Events>,
    running: JoinSet<()>,
    // Handlers `abandon` cancelled, each as the timeout it reports.
    abandoned: Arc<Mutex<Vec<HandlerError>>>,
}

impl Tasks {
    pub(crate) fn new(ordered: bool, permits: Option<Arc<Semaphore>>) -> Self {
        Tasks {
            lanes: ordered.then(Lanes::default),
            limit: permits.as_ref().map(|p| p.available_permits().saturating_add(MAX_WAITING_HANDLERS)),
            permits,
            on_error: None,
            events: Arc::default(),
            running: JoinSet::new(),
            abandoned: Arc::default(),
        }
    }

    /// Waits for every handler started so far.
    pub(crate) async fn finish(&mut self) {
        while self.running.join_next().await.is_some() {}
    }

    /// Cancels the handlers still running, then reports each as one that returned `Error::Timeout`: to `on_error`
    /// while `REPORT_GRACE` lasts, and past it, so an `on_error` that hangs can't hold `stop` up again, as
    /// `HandlerFailed`, the one `on_error` was given and didn't finish with included.
    pub(crate) async fn abandon(&mut self) {
        self.running.abort_all();
        while self.running.join_next().await.is_some() {}
        let abandoned = std::mem::take(&mut *self.abandoned.lock().unwrap_or_else(PoisonError::into_inner));
        let until = Instant::now() + REPORT_GRACE;
        for error in abandoned {
            let error = match &self.on_error {
                Some(on_error) if Instant::now() < until => {
                    let again = like(&error, Error::Timeout);
                    match tokio::time::timeout_at(until, on_error(error)).await {
                        Ok(()) => continue,
                        Err(_) => again,
                    }
                }
                _ => error,
            };
            self.events.warn(ClientEvent::HandlerFailed { error: Arc::new(error) });
        }
    }

    /// Whether so many handlers wait for a slot that no more updates should be taken until one ends. A task waiting
    /// behind its own chat's waits for that chat, not for a slot, so it doesn't count.
    pub(crate) fn full(&mut self) -> bool {
        let Some(limit) = self.limit else { return false };
        while self.running.try_join_next().is_some() {}
        let queued = self.lanes.as_ref().map_or(0, |lanes| lanes.queued.load(Ordering::Relaxed));
        self.running.len().saturating_sub(queued) >= limit
    }

    /// Returns once a started handler task ends.
    pub(crate) async fn one_done(&mut self) {
        self.running.join_next().await;
    }

    /// Runs `task`; a panic in it is caught and reported as `HandlerPanicked` for the kind and chat of `timed_out`,
    /// which is what is reported if `abandon` cancels it before it returns. The error it returns is reported after
    /// that, still holding its slot and its chat's turn, so its chat's next update waits for the report; cancelled
    /// while `on_error` has it, it goes out as `HandlerFailed` instead.
    fn spawn(&mut self, timed_out: HandlerError, task: impl Future<Output = Failed> + Send + 'static) {
        while self.running.try_join_next().is_some() {}
        let (kind, chat_id) = (timed_out.kind, timed_out.chat_id);
        let (on_error, events) = (self.on_error.clone(), self.events.clone());
        let mut abandoned = Abandoned(Some(timed_out), self.abandoned.clone());
        self.running.spawn(async move {
            let failed = caught(task).await;
            // The handler has returned: cancelled from here on, it has nothing left to report but its own error.
            abandoned.disarm();
            match failed {
                Ok(Some((error, _held))) => {
                    let copy = on_error.as_ref().map(|_| like(&error, error.error.duplicate()));
                    let mut unreported = Unreported(copy, events.clone());
                    if let Err(payload) = caught(report(on_error, events.clone(), error)).await {
                        events.emit(panicked(kind, chat_id, payload));
                    }
                    unreported.reported();
                }
                Ok(None) => {}
                Err(payload) => events.emit(panicked(kind, chat_id, payload)),
            }
        });
    }

    pub(crate) fn run<T: Routed>(&mut self, kind: UpdateKind, item: T, callbacks: Vec<Callback<T>>) {
        let turn = match self.lanes.as_mut().zip(item.order_key()) {
            Some((lanes, key)) => {
                let Some(turn) = lanes.turn(key, self.limit.is_some()) else {
                    // The chat's backlog, or every chat's, is full: the update can't wait in the update queue for its
                    // chat alone, and no fetch brings it back.
                    self.events.emit(ClientEvent::UpdatesDropped { count: 1, unrecoverable: 1 });
                    return;
                };
                Some(turn)
            }
            None => None,
        };
        let permits = self.permits.clone();
        let chat_id = item.chat_id();
        let (message_id, user_id, query_id) = (item.message_id(), item.user_id(), item.query_id());
        let failed = move |error| HandlerError { kind, chat_id, message_id, user_id, query_id, error };
        self.spawn(failed(Error::Timeout), async move {
            let done = match turn {
                Some((behind, done)) => {
                    if let Some(mut behind) = behind {
                        let _ = (&mut behind.previous).await;
                    }
                    Some(done)
                }
                None => None,
            };
            let slot = permit(permits).await;
            let (last, rest) = callbacks.split_last()?;
            let item = item.prepare().await;
            for callback in rest {
                if let Err(error) = callback(item.clone()).await {
                    return Some((failed(error), (slot, done)));
                }
            }
            last(item).await.err().map(|error| (failed(error), (slot, done)))
        });
    }

    pub(crate) fn raw(&mut self, future: BoxFuture) {
        let permits = self.permits.clone();
        self.spawn(HandlerError::bare(UpdateKind::Raw, Error::Timeout), async move {
            let slot = permit(permits).await;
            let failed = |error| HandlerError::bare(UpdateKind::Raw, error);
            future.await.err().map(|error| (failed(error), (slot, None::<oneshot::Sender<()>>)))
        });
    }
}

// Handlers outlive the dispatcher unless `finish` waited for them, as before it tracked them.
impl Drop for Tasks {
    fn drop(&mut self) {
        self.running.detach_all();
    }
}

async fn permit(permits: Option<Arc<Semaphore>>) -> Option<OwnedSemaphorePermit> {
    Some(permits?.acquire_owned().await.expect("the semaphore is never closed"))
}

/// Awaits a lifecycle hook; an `Err` or a panic goes where a handler's would.
pub(crate) async fn run_hook(client: Client, kind: UpdateKind, hook: Callback<Client>) {
    // Called inside `caught`, so a panic before the hook's first await is caught too.
    match caught(async { hook(client.clone()).await }).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            let on_error = client.0.handlers.current().on_error.clone();
            report(on_error, client.0.events.clone(), HandlerError::bare(kind, error)).await;
        }
        Err(payload) => client.0.events.emit(panicked(kind, None, payload)),
    }
}

/// Runs user code the dispatcher calls synchronously; a panic is reported as `HandlerPanicked` and gives `None`, so
/// the dispatcher carries on with the next handler and update.
pub(crate) fn guard<R>(events: &Events, kind: UpdateKind, chat_id: Option<i64>, f: impl FnOnce() -> R) -> Option<R> {
    catch_unwind(AssertUnwindSafe(f)).map_err(|payload| events.emit(panicked(kind, chat_id, payload))).ok()
}

/// Awaits `future`, turning a panic in any of its polls into `Err` with the panic's payload. The future is dropped
/// before this returns, so the lane and permit it holds are given back.
async fn caught<F: Future>(future: F) -> Result<F::Output, Box<dyn Any + Send>> {
    let mut future = std::pin::pin!(future);
    std::future::poll_fn(|cx| match catch_unwind(AssertUnwindSafe(|| future.as_mut().poll(cx))) {
        Ok(Poll::Ready(output)) => Poll::Ready(Ok(output)),
        Ok(Poll::Pending) => Poll::Pending,
        Err(payload) => Poll::Ready(Err(payload)),
    })
    .await
}

fn panicked(kind: UpdateKind, chat_id: Option<i64>, payload: Box<dyn Any + Send>) -> ClientEvent {
    let message = match payload.downcast::<String>() {
        Ok(message) => Some(*message),
        Err(payload) => payload.downcast_ref::<&str>().map(|message| (*message).to_owned()),
    };
    ClientEvent::HandlerPanicked { kind, chat_id, message }
}

async fn report(on_error: Option<ErrorCallback>, events: Arc<Events>, error: HandlerError) {
    match on_error {
        Some(on_error) => on_error(error).await,
        None => events.warn(ClientEvent::HandlerFailed { error: Arc::new(error) }),
    }
}

/// How long `abandon` gives `on_error` for the reports of the handlers it cancelled, all together.
const REPORT_GRACE: Duration = Duration::from_secs(5);

/// The error a handler task returned, with its slot and its chat's turn, which it holds until the error is reported.
type Failed = Option<(HandlerError, (Option<OwnedSemaphorePermit>, Option<oneshot::Sender<()>>))>;

/// `error` with another error in it, for a second report of the same handler.
fn like(error: &HandlerError, other: Error) -> HandlerError {
    let (kind, chat_id, message_id) = (error.kind, error.chat_id, error.message_id);
    HandlerError { kind, chat_id, message_id, user_id: error.user_id, query_id: error.query_id, error: other }
}

/// Puts the error it holds in `abandon`'s list when it is dropped still holding it: the task it is in was cancelled.
struct Abandoned(Option<HandlerError>, Arc<Mutex<Vec<HandlerError>>>);

impl Drop for Abandoned {
    fn drop(&mut self) {
        if let Some(error) = self.0.take() {
            self.1.lock().unwrap_or_else(PoisonError::into_inner).push(error);
        }
    }
}

impl Abandoned {
    /// The handler returned on its own: nothing to report when it drops.
    fn disarm(&mut self) {
        self.0 = None;
    }
}

/// A copy of the error `on_error` is reporting, sent as `HandlerFailed` when it is dropped still holding it: the
/// task was cancelled before `on_error` finished, so the error would otherwise be lost.
struct Unreported(Option<HandlerError>, Arc<Events>);

impl Drop for Unreported {
    fn drop(&mut self) {
        if let Some(error) = self.0.take() {
            self.1.warn(ClientEvent::HandlerFailed { error: Arc::new(error) });
        }
    }
}

impl Unreported {
    /// `on_error` finished, or panicked and was reported so: nothing to send when it drops.
    fn reported(&mut self) {
        self.0 = None;
    }
}

#[derive(Default)]
struct Lanes {
    tails: HashMap<i64, Lane>,
    pruned_at: usize,
    // Tasks waiting behind the one before them in their chat, over every chat.
    queued: Arc<AtomicUsize>,
}

struct Lane {
    tail: oneshot::Receiver<()>,
    // Tasks waiting behind the one before them in this chat.
    queued: Arc<AtomicUsize>,
}

impl Lanes {
    /// The place of a task of `key`'s chat behind the one before it, if any, and what ends its own turn when dropped.
    /// `None` when `bounded` and `MAX_WAITING_HANDLERS` already wait in the chat, or `MAX_WAITING_IN_LANES` in all.
    fn turn(&mut self, key: i64, bounded: bool) -> Option<(Option<Behind>, oneshot::Sender<()>)> {
        let (behind, done) = match self.tails.get_mut(&key) {
            Some(lane) => {
                let (in_chat, in_all) = (lane.queued.load(Ordering::Relaxed), self.queued.load(Ordering::Relaxed));
                if bounded && (in_chat >= MAX_WAITING_HANDLERS || in_all >= MAX_WAITING_IN_LANES) {
                    return None;
                }
                let (done, next) = oneshot::channel();
                let previous = std::mem::replace(&mut lane.tail, next);
                (Some(Behind::new(previous, &lane.queued, &self.queued)), done)
            }
            None => {
                let (done, next) = oneshot::channel();
                self.tails.insert(key, Lane { tail: next, queued: Arc::default() });
                (None, done)
            }
        };
        if self.tails.len() >= 2 * self.pruned_at.max(64) {
            self.tails.retain(|_, lane| matches!(lane.tail.try_recv(), Err(oneshot::error::TryRecvError::Empty)));
            self.pruned_at = self.tails.len();
        }
        Some((behind, done))
    }
}

/// A task's place behind the one before it in its chat, counted as waiting there until dropped.
struct Behind {
    previous: oneshot::Receiver<()>,
    lane: Arc<AtomicUsize>,
    all: Arc<AtomicUsize>,
}

impl Behind {
    fn new(previous: oneshot::Receiver<()>, lane: &Arc<AtomicUsize>, all: &Arc<AtomicUsize>) -> Self {
        lane.fetch_add(1, Ordering::Relaxed);
        all.fetch_add(1, Ordering::Relaxed);
        Behind { previous, lane: lane.clone(), all: all.clone() }
    }
}

impl Drop for Behind {
    fn drop(&mut self) {
        self.lane.fetch_sub(1, Ordering::Relaxed);
        self.all.fetch_sub(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lanes_prune_finished_chats() {
        let mut lanes = Lanes::default();
        for key in 0..200 {
            drop(lanes.turn(key, false));
        }
        assert!(lanes.tails.len() < 128);
        let (_, alive) = lanes.turn(1000, false).expect("an unbounded chat always has room");
        for key in 0..200 {
            drop(lanes.turn(key, false));
        }
        assert!(lanes.tails.contains_key(&1000));
        assert_eq!(lanes.queued.load(Ordering::Relaxed), 0);
        drop(alive);
    }

    fn stuck() -> Callback<crate::types::CallbackQuery> {
        use crate::types::CallbackQuery;
        Arc::new(|_: CallbackQuery| -> BoxFuture { Box::pin(std::future::pending::<crate::Result<()>>()) })
    }

    #[tokio::test]
    async fn one_chats_backlog_neither_fills_the_waiting_room_nor_outgrows_its_own_bound() {
        use crate::dispatcher::listeners::tests::query;
        use crate::types::CallbackQuery;

        let client = Client::offline();
        let mut tasks = Tasks::new(true, Some(Arc::new(Semaphore::new(2))));
        let events = tasks.events.record();
        // Chat 7's first handler never ends, so the chat's later updates all wait behind it.
        for _ in 0..=MAX_WAITING_HANDLERS {
            tasks.run(UpdateKind::CallbackQuery, query(&client, 7, 7, 5), vec![stuck()]);
        }
        assert!(!tasks.full(), "one chat's backlog filled the room every chat waits in");
        assert!(events.try_recv().is_err());
        // Past its own bound the chat's next update is dropped and reported, and the room stays open.
        tasks.run(UpdateKind::CallbackQuery, query(&client, 7, 7, 5), vec![stuck()]);
        let dropped = events.try_recv().expect("the dropped update wasn't reported");
        assert!(matches!(dropped, ClientEvent::UpdatesDropped { count: 1, unrecoverable: 1 }), "{dropped:?}");
        assert!(!tasks.full(), "one chat's backlog filled the room every chat waits in");

        // Another chat still gets its turn and a slot.
        let (ran, mut got) = tokio::sync::mpsc::unbounded_channel();
        let other: Callback<CallbackQuery> = Arc::new(move |_: CallbackQuery| -> BoxFuture {
            let _ = ran.send(());
            Box::pin(async { Ok(()) })
        });
        tasks.run(UpdateKind::CallbackQuery, query(&client, 8, 8, 5), vec![other]);
        assert!(got.recv().await.is_some(), "the other chat's handler didn't run");
    }

    #[tokio::test]
    async fn backlogs_over_every_chat_are_bounded_too_and_never_close_the_room() {
        use crate::dispatcher::listeners::tests::query;

        let client = Client::offline();
        let mut tasks = Tasks::new(true, Some(Arc::new(Semaphore::new(2))));
        let events = tasks.events.record();
        // Each of these chats' first handler never ends, and as many as one chat may hold wait behind it.
        let chats = (MAX_WAITING_IN_LANES / MAX_WAITING_HANDLERS) as i64;
        for chat in 1..=chats {
            for _ in 0..=MAX_WAITING_HANDLERS {
                tasks.run(UpdateKind::CallbackQuery, query(&client, chat, chat, 5), vec![stuck()]);
            }
        }
        assert!(events.try_recv().is_err());
        // Another chat's first update waits for no chat and goes; its next would wait behind it, and is dropped.
        let next = chats + 1;
        tasks.run(UpdateKind::CallbackQuery, query(&client, next, next, 5), vec![stuck()]);
        assert!(events.try_recv().is_err());
        tasks.run(UpdateKind::CallbackQuery, query(&client, next, next, 5), vec![stuck()]);
        let dropped = events.try_recv().expect("the dropped update wasn't reported");
        assert!(matches!(dropped, ClientEvent::UpdatesDropped { count: 1, unrecoverable: 1 }), "{dropped:?}");
        assert!(!tasks.full(), "the chats' backlogs filled the room every chat waits in");
    }

    #[tokio::test]
    async fn abandoned_handlers_are_cancelled_and_each_reported_as_timed_out() {
        use crate::dispatcher::listeners::tests::query;

        let client = Client::offline();
        let mut tasks = Tasks::new(true, None);
        let (handled, mut taken) = tokio::sync::mpsc::unbounded_channel();
        let on_error = move |e: HandlerError| -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
            let _ = handled.send(e);
            Box::pin(async {})
        };
        tasks.on_error = Some(Arc::new(on_error));
        // Chat 7's handler never ends and the chat's next one waits behind it; a raw handler never ends either.
        tasks.run(UpdateKind::CallbackQuery, query(&client, 7, 7, 5), vec![stuck()]);
        tasks.run(UpdateKind::CallbackQuery, query(&client, 7, 7, 5), vec![stuck()]);
        tasks.raw(Box::pin(std::future::pending::<crate::Result<()>>()));
        tokio::task::yield_now().await;
        tasks.abandon().await;

        assert!(tasks.running.is_empty());
        assert_eq!(tasks.lanes.as_ref().map(|lanes| lanes.queued.load(Ordering::Relaxed)), Some(0));
        let got: Vec<HandlerError> = std::iter::from_fn(|| taken.try_recv().ok()).collect();
        assert_eq!(got.len(), 3, "{got:?}");
        assert!(got.iter().all(|e| matches!(e.error, Error::Timeout)), "{got:?}");
        let in_chat = |e: &&HandlerError| (e.kind, e.chat_id) == (UpdateKind::CallbackQuery, Some(7));
        assert_eq!(got.iter().filter(in_chat).count(), 2, "{got:?}");
    }

    // A hanging `on_error`: it records whether each error it got was a timeout, then never returns.
    fn hanging_on_error(tasks: &mut Tasks) -> tokio::sync::mpsc::UnboundedReceiver<bool> {
        let (handled, taken) = tokio::sync::mpsc::unbounded_channel();
        let on_error = move |e: HandlerError| -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
            let _ = handled.send(matches!(e.error, Error::Timeout));
            Box::pin(std::future::pending())
        };
        tasks.on_error = Some(Arc::new(on_error));
        taken
    }

    #[tokio::test(start_paused = true)]
    async fn a_handler_cancelled_while_on_error_reports_it_still_has_its_own_error_reported_once() {
        let mut tasks = Tasks::default();
        let events = tasks.events.record();
        let (handled, mut taken) = tokio::sync::mpsc::unbounded_channel();
        let on_error = move |e: HandlerError| -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
            let _ = handled.send(matches!(e.error, Error::Timeout));
            Box::pin(tokio::time::sleep(Duration::from_secs(10)))
        };
        tasks.on_error = Some(Arc::new(on_error));
        tasks.raw(Box::pin(async { Err::<(), _>(Error::Unexpected("boom")) }));
        // The handler returned its error, and `on_error` is still reporting it when the deadline comes.
        assert_eq!(taken.recv().await, Some(false));
        let abandoned = tokio::time::timeout(Duration::from_secs(60), tasks.abandon()).await;
        assert!(abandoned.is_ok(), "abandon hung");
        assert!(taken.try_recv().is_err(), "on_error was called again");
        let got: Vec<ClientEvent> = events.try_iter().collect();
        let [ClientEvent::HandlerFailed { error }] = &got[..] else { panic!("{got:?}") };
        assert!(matches!(error.error, Error::Unexpected("boom")), "{got:?}");
    }

    #[tokio::test(start_paused = true)]
    async fn abandon_gives_a_hanging_on_error_a_grace_then_reports_as_events() {
        let mut tasks = Tasks::default();
        let events = tasks.events.record();
        let mut taken = hanging_on_error(&mut tasks);
        tasks.raw(Box::pin(std::future::pending::<crate::Result<()>>()));
        tasks.raw(Box::pin(std::future::pending::<crate::Result<()>>()));
        let abandoned = tokio::time::timeout(REPORT_GRACE * 2, tasks.abandon()).await;
        assert!(abandoned.is_ok(), "a hanging on_error held abandon up");
        // `on_error` got the first and never returned; past the grace both went out as events.
        assert_eq!(std::iter::from_fn(|| taken.try_recv().ok()).collect::<Vec<_>>(), [true]);
        let got: Vec<Option<UpdateKind>> = events.try_iter().map(|e| timed_out(&e)).collect();
        assert_eq!(got, [Some(UpdateKind::Raw); 2]);
    }

    /// The kind of a `HandlerFailed` that reports a timeout.
    fn timed_out(event: &ClientEvent) -> Option<UpdateKind> {
        let ClientEvent::HandlerFailed { error } = event else { return None };
        matches!(error.error, Error::Timeout).then_some(error.kind)
    }

    #[tokio::test]
    async fn errors_go_to_on_error_or_else_to_events() {
        use crate::errors::Error;
        let mut tasks = Tasks::default();
        let events = tasks.events.record();
        tasks.raw(Box::pin(async { Err::<(), _>(Error::Unexpected("boom")) }));
        let event = loop {
            match events.try_recv() {
                Ok(e) => break e,
                Err(_) => tokio::task::yield_now().await,
            }
        };
        let failed = HandlerError::bare(UpdateKind::Raw, Error::Unexpected("boom"));
        assert_eq!(format!("{event:?}"), format!("{:?}", ClientEvent::HandlerFailed { error: Arc::new(failed) }));

        let (handled, mut taken) = tokio::sync::mpsc::unbounded_channel();
        let on_error = move |e: HandlerError| -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
            let _ = handled.send(e.kind);
            Box::pin(async {})
        };
        tasks.on_error = Some(Arc::new(on_error));
        tasks.raw(Box::pin(async { Err::<(), _>(Error::Unexpected("boom")) }));
        assert_eq!(taken.recv().await, Some(UpdateKind::Raw));
        assert!(events.try_recv().is_err());
    }

    #[tokio::test]
    async fn a_failing_callback_handler_reports_its_query_and_user() {
        use crate::dispatcher::listeners::tests::query;
        use crate::errors::Error;
        use crate::types::CallbackQuery;

        let client = Client::offline();
        // The press helper builds query 1, from user 7, on message 5 of chat 7.
        let press = query(&client, 7, 7, 5);
        let mut tasks = Tasks::default();
        let (handled, mut taken) = tokio::sync::mpsc::unbounded_channel();
        let on_error = move |e: HandlerError| -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
            let _ = handled.send(e);
            Box::pin(async {})
        };
        tasks.on_error = Some(Arc::new(on_error));
        let failing: Callback<CallbackQuery> =
            Arc::new(|_: CallbackQuery| -> BoxFuture { Box::pin(async { Err::<(), _>(Error::Unexpected("boom")) }) });
        tasks.run(UpdateKind::CallbackQuery, press, vec![failing]);

        let error = taken.recv().await.expect("no error came");
        assert_eq!((error.query_id, error.user_id, error.message_id), (Some(1), Some(7), Some(5)));
        assert_eq!(error.chat_id, Some(7));
    }

    #[tokio::test]
    async fn a_failing_membership_handler_reports_the_user_who_made_the_change() {
        use crate::dispatcher::Entities;
        use crate::errors::Error;
        use crate::raw::{base, types};
        use crate::types::ChatMemberUpdated;

        let client = Client::offline();
        // User 1 changed the membership of user 2, so the update's user is 1, not 2.
        let creator = types::ChatParticipantCreator { user_id: 2, rank: None };
        let raw = types::UpdateChatParticipant {
            chat_id: 42,
            date: 100,
            actor_id: 1,
            user_id: 2,
            prev_participant: None,
            new_participant: Some(base::ChatParticipant::ChatParticipantCreator(Box::new(creator))),
            invite: None,
            qts: 1,
        };
        let change = ChatMemberUpdated::parse_chat(&client, raw, &Entities::default());
        assert_eq!(change.new_chat_member.as_ref().map(|member| member.user.id), Some(2));

        let mut tasks = Tasks::default();
        let (handled, mut taken) = tokio::sync::mpsc::unbounded_channel();
        let on_error = move |e: HandlerError| -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
            let _ = handled.send(e);
            Box::pin(async {})
        };
        tasks.on_error = Some(Arc::new(on_error));
        let failing: Callback<ChatMemberUpdated> = Arc::new(|_: ChatMemberUpdated| -> BoxFuture {
            Box::pin(async { Err::<(), _>(Error::Unexpected("boom")) })
        });
        tasks.run(UpdateKind::ChatMemberUpdated, change, vec![failing]);

        let error = taken.recv().await.expect("no error came");
        assert_eq!((error.user_id, error.chat_id), (Some(1), Some(-42)));
    }

    #[tokio::test]
    async fn other_errors_reach_on_error_and_downcast() {
        use crate::errors::Error;

        let mut tasks = Tasks::default();
        let (handled, mut taken) = tokio::sync::mpsc::unbounded_channel();
        let on_error = move |e: HandlerError| -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
            let _ = handled.send(e.error);
            Box::pin(async {})
        };
        tasks.on_error = Some(Arc::new(on_error));

        tasks.raw(Box::pin(async { Err::<(), _>(Error::other("boom")) }));
        let error = taken.recv().await.expect("no error came");
        // A string is boxed as std's own string error, so it reads back as text, not by downcast.
        assert_eq!(error.to_string(), "boom");
        assert!(error.downcast_ref::<std::io::Error>().is_none());

        let denied = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        tasks.raw(Box::pin(async move { Err::<(), _>(Error::other(denied)) }));
        let error = taken.recv().await.expect("no error came");
        let io = error.downcast_ref::<std::io::Error>().expect("not an io error");
        assert_eq!(io.kind(), std::io::ErrorKind::PermissionDenied);
    }

    async fn next_event(events: &std::sync::mpsc::Receiver<ClientEvent>) -> ClientEvent {
        for _ in 0..1000 {
            match events.try_recv() {
                Ok(e) => return e,
                Err(_) => tokio::task::yield_now().await,
            }
        }
        panic!("no event came")
    }

    fn panicked_as(event: &ClientEvent) -> Option<(UpdateKind, Option<i64>, Option<&str>)> {
        match event {
            ClientEvent::HandlerPanicked { kind, chat_id, message } => Some((*kind, *chat_id, message.as_deref())),
            _ => None,
        }
    }

    async fn panics_after_a_yield() -> crate::Result<()> {
        tokio::task::yield_now().await;
        panic!("raw {}", 1)
    }

    async fn panics_with_a_number() -> crate::Result<()> {
        std::panic::panic_any(5)
    }

    #[tokio::test]
    async fn panics_in_raw_futures_and_hooks_are_reported() {
        let mut tasks = Tasks::default();
        let events = tasks.events.record();
        tasks.raw(Box::pin(panics_after_a_yield()));
        assert_eq!(panicked_as(&next_event(&events).await), Some((UpdateKind::Raw, None, Some("raw 1"))));
        // A payload that isn't a string has no message.
        tasks.raw(Box::pin(panics_with_a_number()));
        assert_eq!(panicked_as(&next_event(&events).await), Some((UpdateKind::Raw, None, None)));

        // The hook panics before returning its future.
        let client = Client::offline();
        let events = client.0.events.record();
        let hook: Callback<Client> = Arc::new(|_: Client| -> BoxFuture { panic!("hook") });
        run_hook(client, UpdateKind::Start, hook).await;
        assert_eq!(panicked_as(&next_event(&events).await), Some((UpdateKind::Start, None, Some("hook"))));
    }
}
