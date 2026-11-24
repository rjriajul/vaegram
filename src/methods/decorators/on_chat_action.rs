use std::future::Future;

use crate::Client;
use crate::errors::Result;
use crate::filters::Filter;
use crate::handlers::{HandlerRef, UpdateKind, callback};
use crate::types::ChatActionUpdated;

impl Client {
    /// Registers a handler for activities shown in chats, such as typing or uploading a photo, that pass
    /// `filter`. Activities [`ChatAction`](crate::enums::ChatAction) has no variant for are skipped; a stopped
    /// streamed draft goes to [`on_message_generation_stopped`](Client::on_message_generation_stopped).
    ///
    /// Returns a [`HandlerRef`] that sets the handler's group, makes it stop later groups, or gives its id.
    pub fn on_chat_action<F, Fut, T>(&self, filter: Filter<ChatActionUpdated>, f: F) -> HandlerRef<'_>
    where
        F: Fn(ChatActionUpdated) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<T>> + Send + 'static,
    {
        self.register(UpdateKind::ChatAction, |r, id| r.chat_action.push(id, filter, callback(f)))
    }
}
