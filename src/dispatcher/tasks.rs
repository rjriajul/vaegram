use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot};
use tokio::task::JoinSet;

use crate::Client;
use crate::enums::ClientEvent;
use crate::handlers::{BoxFuture, Callback, ErrorCallback, HandlerError, Routed, UpdateKind};
use crate::session::Events;

#[derive(Default)]
pub(crate) struct Tasks {
    lanes: Option<Lanes>,
    permits: Option<Arc<Semaphore>>,
    pub(crate) on_error: Option<ErrorCallback>,
    pub(crate) events: Arc<Events>,
    running: JoinSet<()>,
}

impl Tasks {
    pub(crate) fn new(ordered: bool, permits: Option<Arc<Semaphore>>) -> Self {
        Tasks {
            lanes: ordered.then(Lanes::default),
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

    fn spawn(&mut self, task: impl Future<Output = ()> + Send + 'static) {
        while self.running.try_join_next().is_some() {}
        self.running.spawn(task);
    }

    pub(crate) fn run<T: Routed>(&mut self, kind: UpdateKind, item: T, callbacks: Vec<Callback<T>>) {
        let turn = self.lanes.as_mut().zip(item.order_key()).map(|(lanes, key)| lanes.turn(key));
        let (permits, on_error, events) = (self.permits.clone(), self.on_error.clone(), self.events.clone());
        self.spawn(async move {
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
            let chat_id = item.chat_id();
            let Some((last, rest)) = callbacks.split_last() else { return };
            for callback in rest {
                if let Err(error) = callback(item.clone()).await {
                    return report(on_error, events, HandlerError { kind, chat_id, error }).await;
                }
            }
            if let Err(error) = last(item).await {
                report(on_error, events, HandlerError { kind, chat_id, error }).await;
            }
        });
    }

    pub(crate) fn raw(&mut self, future: BoxFuture) {
        let (permits, on_error, events) = (self.permits.clone(), self.on_error.clone(), self.events.clone());
        self.spawn(async move {
            let _permit = permit(permits).await;
            if let Err(error) = future.await {
                report(on_error, events, HandlerError { kind: UpdateKind::Raw, chat_id: None, error }).await;
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

/// Awaits a lifecycle hook; an `Err` goes where a handler's would.
pub(crate) async fn run_hook(client: Client, kind: UpdateKind, hook: Callback<Client>) {
    if let Err(error) = hook(client.clone()).await {
        let on_error = client.0.handlers.current().on_error.clone();
        report(on_error, client.0.events.clone(), HandlerError { kind, chat_id: None, error }).await;
    }
}

async fn report(on_error: Option<ErrorCallback>, events: Arc<Events>, error: HandlerError) {
    match on_error {
        Some(on_error) => on_error(error).await,
        None => {
            match error.chat_id {
                Some(chat) => eprintln!("vaegram: {:?} handler failed in chat {chat}: {}", error.kind, error.error),
                None => eprintln!("vaegram: {:?} handler failed: {}", error.kind, error.error),
            }
            events.emit(ClientEvent::HandlerFailed { error });
        }
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
        let failed = HandlerError { kind: UpdateKind::Raw, chat_id: None, error: Error::Unexpected("boom") };
        assert_eq!(format!("{event:?}"), format!("{:?}", ClientEvent::HandlerFailed { error: failed }));

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
}
