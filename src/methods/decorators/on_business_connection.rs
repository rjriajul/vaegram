use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::BusinessConnection;

impl Client {
    pub fn on_business_connection<F, Fut, T>(&self, filter: Filter<BusinessConnection>, f: F) -> HandlerRef<'_>
    where
        F: Fn(BusinessConnection) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::BusinessConnection, |r, id| r.business_connection.push(id, filter, callback(f)))
    }
}
