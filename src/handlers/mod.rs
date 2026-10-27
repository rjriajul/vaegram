use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::raw::base;
use crate::types::Message;

pub(crate) type BoxFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;

#[derive(Clone)]
pub(crate) enum Handler {
    Message(Filter, Arc<dyn Fn(Message) -> BoxFuture + Send + Sync>),
    RawUpdate(Arc<dyn Fn(Client, base::Update) -> BoxFuture + Send + Sync>),
}
