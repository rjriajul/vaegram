use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::raw::base;
use crate::types::{CallbackQuery, InlineQuery, Message};

pub(crate) type BoxFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;
pub(crate) type Callback<T> = Arc<dyn Fn(T) -> BoxFuture + Send + Sync>;

pub(crate) fn callback<T, F, Fut>(f: F) -> Callback<T>
where
    F: Fn(T) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    Arc::new(move |value| Box::pin(f(value)) as BoxFuture)
}

#[derive(Clone)]
pub(crate) enum Handler {
    Message(Filter<Message>, Callback<Message>),
    EditedMessage(Filter<Message>, Callback<Message>),
    CallbackQuery(Filter<CallbackQuery>, Callback<CallbackQuery>),
    InlineQuery(Filter<InlineQuery>, Callback<InlineQuery>),
    RawUpdate(Arc<dyn Fn(Client, base::Update) -> BoxFuture + Send + Sync>),
}

pub(crate) enum Event {
    Message(Message),
    EditedMessage(Message),
    CallbackQuery(CallbackQuery),
    InlineQuery(InlineQuery),
}

impl Event {
    pub(crate) fn run(event: &mut Option<Event>, handler: &Handler) -> Option<BoxFuture> {
        let matched = match (event.as_ref()?, handler) {
            (Event::Message(m), Handler::Message(f, _)) | (Event::EditedMessage(m), Handler::EditedMessage(f, _)) => {
                f.check(m)
            }
            (Event::CallbackQuery(q), Handler::CallbackQuery(f, _)) => f.check(q),
            (Event::InlineQuery(q), Handler::InlineQuery(f, _)) => f.check(q),
            _ => false,
        };
        if !matched {
            return None;
        }
        match (event.take()?, handler) {
            (Event::Message(m), Handler::Message(_, cb)) | (Event::EditedMessage(m), Handler::EditedMessage(_, cb)) => {
                Some(cb(m))
            }
            (Event::CallbackQuery(q), Handler::CallbackQuery(_, cb)) => Some(cb(q)),
            (Event::InlineQuery(q), Handler::InlineQuery(_, cb)) => Some(cb(q)),
            _ => None,
        }
    }
}
