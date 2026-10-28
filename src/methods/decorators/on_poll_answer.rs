use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::PollAnswer;

impl Client {
    pub fn on_poll_answer<F, Fut, T>(&self, filter: Filter<PollAnswer>, f: F) -> HandlerRef<'_>
    where
        F: Fn(PollAnswer) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::PollAnswer, |r, id| r.poll_answer.push(id, filter, callback(f)))
    }
}
