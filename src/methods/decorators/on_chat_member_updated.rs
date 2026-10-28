use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::ChatMemberUpdated;

impl Client {
    pub fn on_chat_member_updated<F, Fut, T>(&self, filter: Filter<ChatMemberUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(ChatMemberUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::ChatMemberUpdated, |r, id| r.chat_member_updated.push(id, filter, callback(f)))
    }
}
