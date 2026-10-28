use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use crate::enums::ClientEvent;
use crate::raw::{base, types};

type Listener = Arc<dyn Fn(ClientEvent) + Send + Sync>;

#[derive(Default)]
pub struct Events {
    listener: RwLock<Option<Listener>>,
    dropped: AtomicU32,
    affected: Mutex<Vec<Affected>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Affected {
    pub(crate) channel_id: Option<i64>,
    pub(crate) pts: i32,
    pub(crate) pts_count: i32,
}

impl Affected {
    pub(crate) fn into_updates(self) -> base::Updates {
        let (pts, pts_count, messages) = (self.pts, self.pts_count, Vec::new());
        let update = match self.channel_id {
            Some(channel_id) => {
                let u = types::UpdateDeleteChannelMessages { channel_id, messages, pts, pts_count };
                base::Update::UpdateDeleteChannelMessages(Box::new(u))
            }
            None => {
                base::Update::UpdateDeleteMessages(Box::new(types::UpdateDeleteMessages { messages, pts, pts_count }))
            }
        };
        base::Updates::UpdateShort(Box::new(types::UpdateShort { update, date: 0 }))
    }
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

    pub(crate) fn affected(&self, a: Affected) {
        if a.pts_count == 0 {
            return;
        }
        let mut affected = self.affected.lock().unwrap();
        match affected.iter_mut().find(|e| e.channel_id == a.channel_id) {
            Some(e) if e.pts == a.pts.wrapping_sub(a.pts_count) => {
                (e.pts, e.pts_count) = (a.pts, e.pts_count.wrapping_add(a.pts_count));
            }
            Some(e) => *e = a,
            None => affected.push(a),
        }
    }

    pub(crate) fn take_affected(&self) -> Option<Affected> {
        self.affected.lock().unwrap().pop()
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

#[cfg(test)]
mod tests {
    use super::{Affected, Events};

    #[test]
    fn affected_pts_merge_per_box() {
        let events = Events::default();
        let a = |channel_id, pts, pts_count| Affected { channel_id, pts, pts_count };
        for affected in [a(None, 11, 1), a(None, 13, 2), a(Some(7), 5, 1), a(None, 20, 1), a(None, 21, 0)] {
            events.affected(affected);
        }
        assert_eq!(*events.affected.lock().unwrap(), [a(None, 20, 1), a(Some(7), 5, 1)]);
    }
}
