use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::UserStatusUpdated;

impl Client {
    pub fn on_user_status<F, Fut, T>(&self, filter: Filter<UserStatusUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(UserStatusUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::UserStatus, |r, id| r.user_status.push(id, filter, callback(f)))
    }
}
