use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::Client;
use crate::errors::Result;
use crate::raw::base;

pub(crate) type BoxFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;
pub(crate) type Callback<T> = Arc<dyn Fn(T) -> BoxFuture + Send + Sync>;
pub(crate) type RawCallback = Arc<dyn Fn(Client, base::Update) -> BoxFuture + Send + Sync>;

pub(crate) fn callback<T, F, Fut>(f: F) -> Callback<T>
where
    F: Fn(T) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    Arc::new(move |value| Box::pin(f(value)) as BoxFuture)
}
