use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::Story;

impl Client {
    pub fn on_story<F, Fut, T>(&self, filter: Filter<Story>, f: F) -> HandlerRef<'_>
    where
        F: Fn(Story) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::Story, |r, id| r.story.push(id, filter, callback(f)))
    }
}
