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
}
