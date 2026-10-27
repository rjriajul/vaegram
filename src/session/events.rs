use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};

use crate::enums::ClientEvent;

type Listener = Arc<dyn Fn(ClientEvent) + Send + Sync>;

/// Shared by a client and its sessions: the `on_event` listener, and how many batches of updates
/// a session dropped because the dispatcher fell behind.
#[derive(Default)]
pub struct Events {
    listener: RwLock<Option<Listener>>,
    dropped: AtomicU32,
}

impl Events {
    pub(crate) fn listen(&self, listener: Listener) {
        *self.listener.write().unwrap() = Some(listener);
    }

    pub(crate) fn emit(&self, event: ClientEvent) {
        let listener = self.listener.read().unwrap().clone();
        if let Some(listener) = listener {
            // A panicking listener would otherwise take the connection down with it; the panic hook still reports it.
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| listener(event)));
        }
    }

    pub(crate) fn drop_updates(&self) {
        self.dropped.fetch_add(1, Ordering::Release);
    }

    pub(crate) fn take_dropped(&self) -> u32 {
        self.dropped.swap(0, Ordering::AcqRel)
    }

    #[cfg(test)]
    pub(crate) fn record(&self) -> std::sync::mpsc::Receiver<ClientEvent> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.listen(Arc::new(move |e| {
            let _ = tx.send(e);
        }));
        rx
    }
}
