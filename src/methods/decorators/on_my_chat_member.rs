use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::ChatMemberUpdated;

impl Client {
    /// The bot's own membership changed: added, removed or promoted in a chat, or blocked or restarted by a user.
    pub fn on_my_chat_member<F, Fut, T>(&self, filter: Filter<ChatMemberUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(ChatMemberUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::MyChatMember, |r, id| r.my_chat_member.push(id, filter, callback(f)))
    }
}
