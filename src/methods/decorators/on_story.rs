use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::Story;

impl Client {
    /// Registers a handler for stories that pass `filter`. Deleted stories never reach it.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_story<F, Fut, T>(&self, filter: Filter<Story>, f: F) -> HandlerRef<'_>
    where
        F: Fn(Story) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::Story, |r, id| r.story.push(id, filter, callback(f)))
    }
}
