use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{OwnedSemaphorePermit, Semaphore, oneshot};

use crate::handlers::{BoxFuture, Callback, ErrorCallback, HandlerError, Routed, UpdateKind};

#[derive(Default)]
pub(crate) struct Tasks {
    lanes: Option<Lanes>,
    permits: Option<Arc<Semaphore>>,
    pub(crate) on_error: Option<ErrorCallback>,
}

impl Tasks {
    pub(crate) fn new(ordered: bool, permits: Option<Arc<Semaphore>>) -> Self {
        Tasks { lanes: ordered.then(Lanes::default), permits, on_error: None }
    }

    pub(crate) fn run<T: Routed>(&mut self, kind: UpdateKind, item: T, callbacks: Vec<Callback<T>>) {
        let turn = self.lanes.as_mut().zip(item.order_key()).map(|(lanes, key)| lanes.turn(key));
        let (permits, on_error) = (self.permits.clone(), self.on_error.clone());
        tokio::spawn(async move {
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
                    return report(on_error, HandlerError { kind, chat_id, error }).await;
                }
            }
            if let Err(error) = last(item).await {
                report(on_error, HandlerError { kind, chat_id, error }).await;
            }
        });
    }

    pub(crate) fn raw(&self, future: BoxFuture) {
        let (permits, on_error) = (self.permits.clone(), self.on_error.clone());
        tokio::spawn(async move {
            let _permit = permit(permits).await;
            if let Err(error) = future.await {
                report(on_error, HandlerError { kind: UpdateKind::Raw, chat_id: None, error }).await;
            }
        });
    }
}

async fn permit(permits: Option<Arc<Semaphore>>) -> Option<OwnedSemaphorePermit> {
    Some(permits?.acquire_owned().await.expect("the semaphore is never closed"))
}

async fn report(on_error: Option<ErrorCallback>, error: HandlerError) {
    match on_error {
        Some(on_error) => on_error(error).await,
        None => match error.chat_id {
            Some(chat) => eprintln!("vaegram: {:?} handler failed in chat {chat}: {}", error.kind, error.error),
            None => eprintln!("vaegram: {:?} handler failed: {}", error.kind, error.error),
        },
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
}
