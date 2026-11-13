use std::any::Any;
use std::collections::HashMap;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::task::Poll;

use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot};
use tokio::task::JoinSet;

use crate::Client;
use crate::enums::ClientEvent;
use crate::handlers::{BoxFuture, Callback, ErrorCallback, HandlerError, Routed, UpdateKind};
use crate::session::Events;

/// With `max_concurrent_handlers`, how many handler tasks may wait for a slot before the dispatcher stops taking
/// updates. Past it, updates wait in the update queue, whose overflow getDifference fetches again, instead of
/// piling up in memory.
pub(crate) const MAX_WAITING_HANDLERS: usize = 1024;

#[derive(Default)]
pub(crate) struct Tasks {
    lanes: Option<Lanes>,
    permits: Option<Arc<Semaphore>>,
    // Started and unfinished tasks past which `full` says so; only with a handler cap.
    limit: Option<usize>,
    pub(crate) on_error: Option<ErrorCallback>,
    pub(crate) events: Arc<Events>,
    running: JoinSet<()>,
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
        }
    }

    /// Waits for every handler started so far.
    pub(crate) async fn finish(&mut self) {
        while self.running.join_next().await.is_some() {}
    }

    /// Whether so many handlers wait for a slot that no more updates should be taken until one ends.
    pub(crate) fn full(&mut self) -> bool {
        let Some(limit) = self.limit else { return false };
        while self.running.try_join_next().is_some() {}
        self.running.len() >= limit
    }

    /// Returns once a started handler task ends.
    pub(crate) async fn one_done(&mut self) {
        self.running.join_next().await;
    }

    /// Runs `task`; a panic in it is caught and reported as `HandlerPanicked` for `kind` and `chat_id`.
    fn spawn(&mut self, kind: UpdateKind, chat_id: Option<i64>, task: impl Future<Output = ()> + Send + 'static) {
        while self.running.try_join_next().is_some() {}
        let events = self.events.clone();
        self.running.spawn(async move {
            if let Err(payload) = caught(task).await {
                events.emit(panicked(kind, chat_id, payload));
            }
        });
    }

    pub(crate) fn run<T: Routed>(&mut self, kind: UpdateKind, item: T, callbacks: Vec<Callback<T>>) {
        let turn = self.lanes.as_mut().zip(item.order_key()).map(|(lanes, key)| lanes.turn(key));
        let (permits, on_error, events) = (self.permits.clone(), self.on_error.clone(), self.events.clone());
        let chat_id = item.chat_id();
        let (message_id, user_id, query_id) = (item.message_id(), item.user_id(), item.query_id());
        let failed = move |error| HandlerError { kind, chat_id, message_id, user_id, query_id, error };
        self.spawn(kind, chat_id, async move {
            let _done = match turn {
                Some((previous, done)) => {
                    if let Some(previous) = previous {
                        let _ = previous.await;
                    }
                    Some(done)
                }
                None => None,
            };
            let _permit = permit(permits).await;
            let Some((last, rest)) = callbacks.split_last() else { return };
            let item = item.prepare().await;
            for callback in rest {
                if let Err(error) = callback(item.clone()).await {
                    return report(on_error, events, failed(error)).await;
                }
            }
            if let Err(error) = last(item).await {
                report(on_error, events, failed(error)).await;
            }
        });
    }

    pub(crate) fn raw(&mut self, future: BoxFuture) {
        let (permits, on_error, events) = (self.permits.clone(), self.on_error.clone(), self.events.clone());
        self.spawn(UpdateKind::Raw, None, async move {
            let _permit = permit(permits).await;
            if let Err(error) = future.await {
                report(on_error, events, HandlerError::bare(UpdateKind::Raw, error)).await;
            }
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

#[derive(Default)]
struct Lanes {
    tails: HashMap<i64, oneshot::Receiver<()>>,
    pruned_at: usize,
}

impl Lanes {
    fn turn(&mut self, key: i64) -> (Option<oneshot::Receiver<()>>, oneshot::Sender<()>) {
        let (done, next) = oneshot::channel();
        let previous = self.tails.insert(key, next);
        if self.tails.len() >= 2 * self.pruned_at.max(64) {
            self.tails.retain(|_, rx| matches!(rx.try_recv(), Err(oneshot::error::TryRecvError::Empty)));
            self.pruned_at = self.tails.len();
        }
        (previous, done)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lanes_prune_finished_chats() {
        let mut lanes = Lanes::default();
        for key in 0..200 {
            drop(lanes.turn(key));
        }
        assert!(lanes.tails.len() < 128);
        let (_, alive) = lanes.turn(1000);
        for key in 0..200 {
            drop(lanes.turn(key));
        }
        assert!(lanes.tails.contains_key(&1000));
        drop(alive);
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
        assert_eq!(error.downcast_ref::<String>().map(String::as_str), Some("boom"));

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
