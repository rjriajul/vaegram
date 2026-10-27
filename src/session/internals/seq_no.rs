#[derive(Default)]
pub(crate) struct SeqNo(i32);

impl SeqNo {
    pub(crate) fn next(&mut self, content_related: bool) -> i32 {
        if content_related {
            self.0 += 1;
            self.0 * 2 - 1
        } else {
            self.0 * 2
        }
    }

    // bad_msg_notification 32 (too low) or 33 (too high). The server doesn't say which
    // seqno it expects, so jump by a heuristic margin.
    pub(crate) fn resync(&mut self, too_low: bool) {
        self.0 = if too_low { self.0 + 64 } else { (self.0 - 16).max(0) };
    }
}
