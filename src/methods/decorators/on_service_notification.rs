use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::ServiceNotification;

impl Client {
    /// Registers a handler for notices from Telegram itself, such as a warning that the account was limited, that
    /// pass `filter`.
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_service_notification<F, Fut, T>(&self, filter: Filter<ServiceNotification>, f: F) -> HandlerRef<'_>
    where
        F: Fn(Client, ServiceNotification) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::ServiceNotification, |r, id| r.service_notification.push(id, filter, callback(f)))
    }
}
