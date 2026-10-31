use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::Client;
use crate::dispatcher::Router;
use crate::errors::{Error, Result};
use crate::types::{
    BotSubscriptionUpdated, BusinessConnection, CallbackQuery, ChatBoostRemoved, ChatBoostUpdated, ChatJoinRequest,
    ChatMemberUpdated, ChosenInlineResult, DeletedBusinessMessages, DeletedEphemeralMessages, DeletedMessages,
    EphemeralMessage, GuestMessage, InlineQuery, ManagedBotUpdated, Message, MessageGenerationStopped,
    MessageReactionCountUpdated, MessageReactionUpdated, PollAnswer, PollUpdated, PreCheckoutQuery, PurchasedPaidMedia,
    RawUpdate, ShippingQuery, UserStatusUpdated,
};

pub use crate::dispatcher::router::UpdateKind;

pub(crate) type BoxFuture = Pin<Box<dyn Future<Output = Result<()>> + Send>>;
pub(crate) type Callback<T> = Arc<dyn Fn(T) -> BoxFuture + Send + Sync>;
pub(crate) type RawCallback = Arc<dyn Fn(Client, RawUpdate) -> BoxFuture + Send + Sync>;
pub(crate) type ErrorCallback = Arc<dyn Fn(HandlerError) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

pub(crate) fn callback<T, F, Fut, R>(f: F) -> Callback<T>
where
    F: Fn(T) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<R>> + Send + 'static,
{
    Arc::new(move |value| {
        let future = f(value);
        Box::pin(async move { future.await.map(drop) }) as BoxFuture
    })
}

#[non_exhaustive]
#[derive(Debug)]
pub struct HandlerError {
    pub kind: UpdateKind,
    pub chat_id: Option<i64>,
    pub error: Error,
}

#[derive(Clone, Copy)]
pub struct HandlerRef<'a> {
    client: &'a Client,
    kind: UpdateKind,
    id: u32,
}

impl HandlerRef<'_> {
    /// Groups run in ascending order, default 0. Within a group the first matching handler
    /// wins, and every group gets its own turn. Raw handlers always run and ignore this.
    pub fn group(self, group: i32) -> Self {
        self.client.0.handlers.add(|r| r.configure(self.kind, self.id, Some(group), false));
        self
    }

    /// When this handler matches, the groups after it don't run for that update.
    pub fn stop(self) -> Self {
        self.client.0.handlers.add(|r| r.configure(self.kind, self.id, None, true));
        self
    }

    /// An id to pass to `Client::remove_handler`, which can be kept after the client borrow ends.
    pub fn id(self) -> HandlerId {
        HandlerId { kind: self.kind, id: self.id }
    }
}

/// One registered handler. Ids are never reused, so a stale one removes nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HandlerId {
    pub(crate) kind: UpdateKind,
    pub(crate) id: u32,
}

impl Client {
    pub(crate) fn register(&self, kind: UpdateKind, add: impl FnOnce(&mut Router, u32)) -> HandlerRef<'_> {
        let id = self.0.handlers.add(|r| {
            let id = r.next_id();
            add(r, id);
            id
        });
        HandlerRef { client: self, kind, id }
    }
}

pub(crate) trait Routed: Clone + Send + 'static {
    fn chat_id(&self) -> Option<i64>;

    fn order_key(&self) -> Option<i64> {
        self.chat_id()
    }
}

impl Routed for Message {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }
}

impl Routed for CallbackQuery {
    fn chat_id(&self) -> Option<i64> {
        self.chat().map(|c| c.id)
    }

    fn order_key(&self) -> Option<i64> {
        Some(self.chat().map_or(self.from_user.id, |c| c.id))
    }
}

impl Routed for InlineQuery {
    fn chat_id(&self) -> Option<i64> {
        None
    }

    fn order_key(&self) -> Option<i64> {
        Some(self.from_user.id)
    }
}

impl Routed for ChosenInlineResult {
    fn chat_id(&self) -> Option<i64> {
        None
    }

    fn order_key(&self) -> Option<i64> {
        Some(self.from_user.id)
    }
}

impl Routed for ChatJoinRequest {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }
}

impl Routed for GuestMessage {
    fn chat_id(&self) -> Option<i64> {
        Some(self.message.chat.id)
    }
}

impl Routed for BotSubscriptionUpdated {
    fn chat_id(&self) -> Option<i64> {
        Some(self.user.id)
    }
}

impl Routed for MessageGenerationStopped {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }
}

impl Routed for ManagedBotUpdated {
    fn chat_id(&self) -> Option<i64> {
        None
    }

    fn order_key(&self) -> Option<i64> {
        Some(self.bot.id)
    }
}

impl Routed for EphemeralMessage {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }
}

impl Routed for MessageReactionUpdated {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }
}

impl Routed for DeletedEphemeralMessages {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }
}

impl Routed for MessageReactionCountUpdated {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }
}

impl Routed for BusinessConnection {
    fn chat_id(&self) -> Option<i64> {
        Some(self.user.id)
    }
}

impl Routed for DeletedBusinessMessages {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }
}

impl Routed for ShippingQuery {
    fn chat_id(&self) -> Option<i64> {
        Some(self.from_user.id)
    }
}

impl Routed for PreCheckoutQuery {
    fn chat_id(&self) -> Option<i64> {
        Some(self.from_user.id)
    }
}

impl Routed for PurchasedPaidMedia {
    fn chat_id(&self) -> Option<i64> {
        Some(self.from_user.id)
    }
}

impl Routed for ChatBoostUpdated {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }
}

impl Routed for ChatBoostRemoved {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }
}

impl Routed for ChatMemberUpdated {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }
}

impl Routed for PollUpdated {
    fn chat_id(&self) -> Option<i64> {
        self.chat.as_ref().map(|c| c.id)
    }

    fn order_key(&self) -> Option<i64> {
        Some(self.poll.id)
    }
}

impl Routed for PollAnswer {
    fn chat_id(&self) -> Option<i64> {
        None
    }

    fn order_key(&self) -> Option<i64> {
        Some(self.poll_id)
    }
}

impl Routed for DeletedMessages {
    fn chat_id(&self) -> Option<i64> {
        self.chat.as_ref().map(|c| c.id)
    }
}

impl Routed for UserStatusUpdated {
    fn chat_id(&self) -> Option<i64> {
        None
    }

    fn order_key(&self) -> Option<i64> {
        Some(self.user.id)
    }
}
