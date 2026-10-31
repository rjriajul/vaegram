use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use tokio::sync::Notify;

use crate::enums::ClientEvent;
use crate::raw::{base, types};

type Listener = Arc<dyn Fn(ClientEvent) + Send + Sync>;
type LinkListener = Arc<dyn Fn(bool) + Send + Sync>;

#[derive(Default)]
pub struct Events {
    listener: RwLock<Option<Listener>>,
    // Told when the main connection goes down (false) or comes back (true).
    link: RwLock<Option<LinkListener>>,
    // Dropped batches in the low half, the unrecoverable ones among them in the high half.
    dropped: AtomicU64,
    reconnected: AtomicBool,
    affected: Mutex<Vec<Affected>>,
    nested: Mutex<VecDeque<base::Updates>>,
    pub(crate) wake: Notify,
}

// Past this many unread batches the oldest is dropped, which asks for a getDifference.
const NESTED_LIMIT: usize = 64;

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

    pub(crate) fn on_link(&self, listener: LinkListener) {
        *self.link.write().unwrap() = Some(listener);
    }

    pub(crate) fn link(&self, up: bool) {
        let listener = self.link.read().unwrap().clone();
        if let Some(listener) = listener {
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| listener(up)));
        }
    }

    pub(crate) fn drop_updates(&self, updates: &base::Updates) {
        self.dropped.fetch_add(1 | (u64::from(unrecoverable(updates)) << 32), Ordering::AcqRel);
    }

    /// Dropped batches, and how many of those held updates getDifference can't bring back.
    pub(crate) fn take_dropped(&self) -> (u32, u32) {
        let dropped = self.dropped.swap(0, Ordering::AcqRel);
        (dropped as u32, (dropped >> 32) as u32)
    }

    pub(crate) fn reconnected(&self) {
        self.reconnected.store(true, Ordering::Release);
    }

    pub(crate) fn take_reconnected(&self) -> bool {
        self.reconnected.swap(false, Ordering::AcqRel)
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

    /// Updates that came inside another reply, to go the way the ones on the update stream go.
    pub(crate) fn nested(&self, updates: base::Updates) {
        let mut nested = self.nested.lock().unwrap();
        if nested.len() == NESTED_LIMIT {
            let lost = nested.pop_front().expect("a full queue has a front");
            self.drop_updates(&lost);
        }
        nested.push_back(updates);
        drop(nested);
        self.wake.notify_one();
    }

    pub(crate) fn take_nested(&self) -> Option<base::Updates> {
        self.nested.lock().unwrap().pop_front()
    }

    #[cfg(test)]
    pub(crate) fn record(&self) -> std::sync::mpsc::Receiver<ClientEvent> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.listen(Arc::new(move |e| {
            let _ = tx.send(e);
        }));
        rx
    }

    #[cfg(test)]
    pub(crate) fn record_link(&self) -> std::sync::mpsc::Receiver<bool> {
        let (tx, rx) = std::sync::mpsc::channel();
        self.on_link(Arc::new(move |up| {
            let _ = tx.send(up);
        }));
        rx
    }
}

// getDifference replays only what moved pts or qts.
fn unrecoverable(updates: &base::Updates) -> bool {
    let lost = |u: &base::Update| u.pts().is_none() && u.qts().is_none();
    match updates {
        base::Updates::UpdateShort(u) => lost(&u.update),
        base::Updates::Updates(u) => u.updates.iter().any(lost),
        base::Updates::UpdatesCombined(u) => u.updates.iter().any(lost),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{Affected, Events, NESTED_LIMIT};
    use crate::raw::base;

    #[test]
    fn affected_pts_merge_per_box() {
        let events = Events::default();
        let a = |channel_id, pts, pts_count| Affected { channel_id, pts, pts_count };
        for affected in [a(None, 11, 1), a(None, 13, 2), a(Some(7), 5, 1), a(None, 20, 1), a(None, 21, 0)] {
            events.affected(affected);
        }
        assert_eq!(*events.affected.lock().unwrap(), [a(None, 20, 1), a(Some(7), 5, 1)]);
    }

    #[test]
    fn unread_nested_updates_are_capped_and_the_overflow_asks_for_a_difference() {
        let events = Events::default();
        for _ in 0..NESTED_LIMIT + 2 {
            events.nested(base::Updates::UpdatesTooLong);
        }
        assert_eq!(events.nested.lock().unwrap().len(), NESTED_LIMIT);
        assert_eq!(events.take_dropped(), (2, 0));
    }
}
