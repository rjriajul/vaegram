use std::collections::VecDeque;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// How far ahead of the server's clock it takes a msg_id: one further ahead is refused as too high (error 17).
const MAX_AHEAD: i64 = 30;
// An id handed out this long ago is behind the server's clock now, even one it took in as far ahead as it takes.
const RECENT: Duration = Duration::from_secs(2 * MAX_AHEAD as u64);
// Runs of ids kept at most: a run only ends where the ids jump, so more of them in a minute is a clock gone wild.
const MAX_RUNS: usize = 16;

pub(crate) fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

pub(crate) struct MsgId {
    last: i64,
    pub(crate) time_offset: i64,
    // The first id of the run `last` ends: ids that rose without jumping ahead by more than the server takes.
    start: Option<i64>,
    // When `last` was handed out.
    last_at: Instant,
    // The runs before, recent ones only: their first and last id, and when the last was handed out.
    runs: VecDeque<(i64, i64, Instant)>,
}

impl MsgId {
    pub(crate) fn new(time_offset: i64) -> Self {
        Self { last: 0, time_offset, start: None, last_at: Instant::now(), runs: VecDeque::new() }
    }

    pub(crate) fn next(&mut self) -> i64 {
        let id = self.clock().max(self.last + 4);
        match self.start {
            // The clock jumped ahead by more than the server takes: the ids before may be all it took in.
            Some(start) if id - self.last > MAX_AHEAD << 32 => {
                self.end_run(start);
                self.start = Some(id);
            }
            Some(_) => {}
            None => self.start = Some(id),
        }
        self.last = id;
        self.last_at = Instant::now();
        id
    }

    // The id the clock gives now.
    fn clock(&self) -> i64 {
        let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        ((t.as_secs() as i64 + self.time_offset) << 32) | ((t.subsec_nanos() as i64) << 2)
    }

    fn end_run(&mut self, start: i64) {
        self.runs.retain(|&(_, _, at)| at.elapsed() < RECENT);
        if self.runs.len() == MAX_RUNS {
            self.runs.pop_front();
        }
        self.runs.push_back((start, self.last, self.last_at));
    }

    pub(crate) fn next_n(&mut self, n: usize) -> i64 {
        debug_assert!(n > 0, "next_n needs at least one id");
        let first = self.next();
        self.last = first + 4 * (n as i64 - 1);
        first
    }

    /// Takes the clock offset the server corrected; when that moved the clock back, returns what every id handed out
    /// from now on is above, which may be below ids handed out before. The server refused every id over 30 s ahead of
    /// its clock, and the offset is in whole seconds, so ids more than 31 s ahead of the corrected clock went nowhere:
    /// the next ids follow that clock and only stay above the ones of each recent run that may have been taken in.
    pub(crate) fn set_time_offset(&mut self, time_offset: i64) -> Option<i64> {
        let back = time_offset < self.time_offset;
        self.time_offset = time_offset;
        if !back {
            return None;
        }
        if let Some(start) = self.start.take() {
            self.end_run(start);
        }
        let refused = (now() + time_offset + MAX_AHEAD + 2) << 32;
        let taken = self.runs.iter().filter(|&&(start, _, at)| start < refused && at.elapsed() < RECENT);
        self.last = taken.map(|&(_, end, _)| end.min(refused - 4)).max().unwrap_or(0);
        Some(self.clock().max(self.last + 4) - 4)
    }

    /// The newest msg_id handed out, or after a clock correction the one the next are above; 0 before the first.
    pub(crate) fn last(&self) -> i64 {
        self.last
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_increase_and_are_divisible_by_4() {
        let mut ids = MsgId::new(0);
        let (a, b) = (ids.next(), ids.next());
        assert!(b > a);
        assert_eq!((a % 4, b % 4), (0, 0));
        assert!(((a >> 32) - now()).abs() <= 1);
    }

    #[test]
    fn a_clock_moved_back_takes_the_next_ids_back_with_it() {
        let mut ids = MsgId::new(600);
        let refused = ids.next();
        let after = ids.set_time_offset(0).unwrap();
        let (a, b) = (ids.next(), ids.next());
        assert!(after < a && a < refused && b > a, "{after} {a} {b} {refused}");
        assert!(((a >> 32) - now()).abs() <= 1);
        // Moved ahead, the clock takes the ids with it by itself.
        assert_eq!(ids.set_time_offset(60), None);
        assert!(ids.next() >> 32 >= now() + 59);
    }

    #[test]
    fn ids_stay_above_the_ones_the_server_may_have_taken_in() {
        let mut ids = MsgId::new(20);
        let taken = ids.next();
        // The local clock jumps ahead, past what the server takes.
        ids.time_offset = 600;
        let refused = ids.next();
        let after = ids.set_time_offset(0).unwrap();
        let next = ids.next();
        assert!(after >= taken && next > after && next < refused, "{taken} {after} {next} {refused}");
    }

    #[test]
    fn an_id_a_second_past_the_edge_still_counts_as_taken_in() {
        // The offset is in whole seconds, so an id the server took in at the edge can read a second past it.
        let mut ids = MsgId::new(31);
        let edge = ids.next();
        ids.time_offset = 600;
        ids.next();
        ids.set_time_offset(0).unwrap();
        assert!(ids.next() > edge);
    }

    #[test]
    fn ids_that_crept_past_the_edge_keep_the_next_at_it() {
        // No jump past what the server takes: the ids crossed the edge, and any up to it may have been taken in.
        let mut ids = MsgId::new(25);
        let taken = ids.next();
        ids.time_offset = 40;
        let refused = ids.next();
        ids.set_time_offset(0).unwrap();
        let next = ids.next();
        assert!(next > taken && next < refused, "{taken} {next} {refused}");
        assert!((next >> 32) - now() <= MAX_AHEAD + 2);
    }
}
