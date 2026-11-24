use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::NewLogin;

impl Client {
    /// Registers a handler for logins to this account from a new device, and their confirmations, that pass
    /// `filter`.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_new_login<F, Fut, T>(&self, filter: Filter<NewLogin>, f: F) -> HandlerRef<'_>
    where
        F: Fn(NewLogin) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::NewLogin, |r, id| r.new_login.push(id, filter, callback(f)))
    }
}
