use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() as i64
}

pub(crate) struct MsgId {
    last: i64,
    pub(crate) time_offset: i64,
}

impl MsgId {
    pub(crate) fn new(time_offset: i64) -> Self {
        Self { last: 0, time_offset }
    }

    pub(crate) fn next(&mut self) -> i64 {
        let t = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
        let id = ((t.as_secs() as i64 + self.time_offset) << 32) | ((t.subsec_nanos() as i64) << 2);
        self.last = id.max(self.last + 4);
        self.last
    }

    pub(crate) fn next_n(&mut self, n: usize) -> i64 {
        let first = self.next();
        self.last = first + 4 * (n as i64 - 1);
        first
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
}
