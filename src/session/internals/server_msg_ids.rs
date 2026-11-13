use std::collections::BTreeSet;

/// The spec's window: a server msg_id may be up to 300 s old or 30 s ahead of us.
const MAX_AGE: i64 = 300;
const MAX_AHEAD: i64 = 30;

/// Replay check for incoming messages, as in
/// <https://core.telegram.org/mtproto/security_guidelines#checking-msg-id>.
#[derive(Default)]
pub(crate) struct ServerMsgIds {
    seen: BTreeSet<i64>,
    // Anything at or below the oldest id we had to forget can't be told apart from a replay.
    floor: i64,
}

impl ServerMsgIds {
    /// How many accepted ids are kept; past that the oldest is forgotten and becomes the floor.
    pub(crate) const REMEMBERED: usize = 2048;

    pub(crate) fn in_window(msg_id: i64, now: i64) -> bool {
        let t = msg_id >> 32;
        now - MAX_AGE <= t && t <= now + MAX_AHEAD
    }

    /// Whether [`accept`](Self::accept) would take `msg_id` now, without recording it.
    pub(crate) fn would_accept(&self, msg_id: i64, now: Option<i64>) -> bool {
        if now.is_some_and(|now| !Self::in_window(msg_id, now)) {
            return false;
        }
        msg_id > self.floor && !self.seen.contains(&msg_id)
    }

    pub(crate) fn accept(&mut self, msg_id: i64, now: Option<i64>) -> bool {
        if !self.would_accept(msg_id, now) {
            return false;
        }
        self.seen.insert(msg_id);
        if self.seen.len() > Self::REMEMBERED {
            self.floor = self.seen.pop_first().unwrap_or(self.floor);
        }
        true
    }

    /// Whether `msg_id` was accepted and is still remembered.
    pub(crate) fn contains(&self, msg_id: i64) -> bool {
        self.seen.contains(&msg_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000;

    #[test]
    fn rejects_duplicates_and_stale_ids() {
        let mut ids = ServerMsgIds::default();
        let id = (NOW << 32) | 1;
        assert!(ids.accept(id, Some(NOW)));
        assert!(!ids.accept(id, Some(NOW)));
        assert!(!ids.accept(((NOW - 301) << 32) | 1, Some(NOW)));
        assert!(!ids.accept(((NOW + 31) << 32) | 1, Some(NOW)));
        assert!(ids.accept(((NOW + 31) << 32) | 1, None));
    }

    #[test]
    fn forgets_oldest_and_rejects_below_it() {
        let mut ids = ServerMsgIds::default();
        let base = NOW << 32;
        for i in 0..=ServerMsgIds::REMEMBERED as i64 {
            assert!(ids.accept(base + 4 * i + 1, Some(NOW)));
        }
        assert_eq!(ids.seen.len(), ServerMsgIds::REMEMBERED);
        assert!(!ids.accept(base + 1, Some(NOW)));
        assert!(!ids.would_accept(base + 1, Some(NOW)) && !ids.contains(base + 1));
        assert!(ids.accept(base + 4 * (ServerMsgIds::REMEMBERED as i64 + 1) + 1, Some(NOW)));
    }

    #[test]
    fn would_accept_records_nothing() {
        let mut ids = ServerMsgIds::default();
        let id = (NOW << 32) | 1;
        assert!(ids.would_accept(id, Some(NOW)) && ids.would_accept(id, Some(NOW)));
        assert!(!ids.contains(id));
        assert!(!ids.would_accept(((NOW - 301) << 32) | 1, Some(NOW)));
        assert!(ids.accept(id, Some(NOW)));
        assert!(ids.contains(id) && !ids.would_accept(id, None));
    }
}
