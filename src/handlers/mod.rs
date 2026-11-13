use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::Client;
use crate::dispatcher::Router;
use crate::errors::{Error, Result};
use crate::types::{
    BotSubscriptionUpdated, BusinessConnection, CallbackQuery, ChatActionUpdated, ChatBoostRemoved, ChatBoostUpdated,
    ChatJoinRequest, ChatMemberUpdated, ChosenInlineResult, DeletedBusinessMessages, DeletedEphemeralMessages,
    DeletedMessages, EphemeralMessage, GuestMessage, InlineQuery, ManagedBotUpdated, Message, MessageGenerationStopped,
    MessageReactionCountUpdated, MessageReactionUpdated, PendingJoinRequestsUpdated, PinnedMessagesUpdated, PollAnswer,
    PollUpdated, PreCheckoutQuery, PurchasedPaidMedia, RawUpdate, ReactionsUpdated, ReadHistoryUpdated,
    SharedWidgetQuery, ShippingQuery, StarsBalanceUpdated, Story, TonWalletState, UserStatusUpdated, WidgetDataUpdated,
    WidgetQuery,
};

pub use crate::dispatcher::listeners::Listenable;
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

/// An `Err` a handler or lifecycle hook returned, as [`Client::on_error`] receives it, or
/// [`ClientEvent::HandlerFailed`](crate::enums::ClientEvent::HandlerFailed) when that isn't set.
#[non_exhaustive]
#[derive(Debug)]
pub struct HandlerError {
    /// The route of the handler that failed: [`UpdateKind::Raw`] for raw handlers, `Start`, `Stop`, `Connect` or
    /// `Disconnect` for the hooks.
    pub kind: UpdateKind,
    /// Bot-API style id of the update's chat. `None` for raw handlers, the hooks and updates without a chat.
    pub chat_id: Option<i64>,
    /// Id of the message the update is about, within [`chat_id`](Self::chat_id): the message a handler failed on,
    /// the message a pressed button or widget is on, the message a reaction changed on, or the message a guest
    /// query answered. `None` for raw handlers, the hooks and updates about no single message.
    pub message_id: Option<i32>,
    /// Bot-API style id of the user who caused the update (its `from_user` or `user`): the sender of a message or
    /// story, the user who pressed the button, sent the query or paid, or the one who changed a membership or
    /// reacted. `None` for raw handlers, the hooks and updates with no user behind them. The one exception is
    /// [`UserStatusUpdated`](crate::types::UserStatusUpdated), where Telegram reports on a user and nobody acted:
    /// there it is that user.
    pub user_id: Option<i64>,
    /// Id of the query the update carries, the one its `answer` method takes: callback, widget, inline, shipping,
    /// pre-checkout, guest and join-request queries. `None` for raw handlers, the hooks and updates without one.
    pub query_id: Option<i64>,
    /// The error the handler returned.
    pub error: Error,
}

impl HandlerError {
    /// A failure with no update behind it: a raw handler's or a lifecycle hook's.
    pub(crate) fn bare(kind: UpdateKind, error: Error) -> Self {
        HandlerError { kind, chat_id: None, message_id: None, user_id: None, query_id: None, error }
    }
}

/// A handler just registered by a `Client::on_*` method: set its group, make it stop later groups, or take its
/// [`HandlerId`]. It borrows the client; dropping it keeps the handler registered.
#[derive(Clone, Copy)]
pub struct HandlerRef<'a> {
    client: &'a Client,
    kind: UpdateKind,
    id: u32,
}

// `group` and `stop` take effect at once; `self` comes back only for chaining, so dropping it is fine.
#[allow(clippy::return_self_not_must_use)]
impl HandlerRef<'_> {
    /// Groups run in ascending order, default 0. Within a group the first matching handler
    /// wins, and every group gets its own turn. Raw handlers always run and ignore this, as do the
    /// lifecycle hooks (`on_start`, `on_stop`, `on_connect`, `on_disconnect`).
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
        HandlerId(Registered::Route(self.kind, self.id))
    }
}

/// One registered handler. Ids are never reused, so a stale one removes nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HandlerId(pub(crate) Registered);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Registered {
    Route(UpdateKind, u32),
    /// A next-step handler, which lives in the listener registry.
    Listener(u64),
}

/// A registered handler, as [`Client::handlers`] lists it.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HandlerInfo {
    /// The id [`Client::remove_handler`] takes.
    pub id: HandlerId,
    /// The route the handler is on, named after the `Client::on_*` method that registered it.
    pub kind: UpdateKind,
    /// The group the handler runs in. `None` for raw handlers and lifecycle hooks, which have no group.
    pub group: Option<i32>,
    /// `true` if a match keeps the later groups from running for that update.
    pub stops: bool,
}

impl HandlerInfo {
    pub(crate) fn new(kind: UpdateKind, id: u32, group: Option<i32>, stops: bool) -> HandlerInfo {
        HandlerInfo { id: HandlerId(Registered::Route(kind, id)), kind, group, stops }
    }
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

    /// Id of the message the update is about, for [`HandlerError::message_id`].
    fn message_id(&self) -> Option<i32> {
        None
    }

    /// Id of the user the update is about, for [`HandlerError::user_id`].
    fn user_id(&self) -> Option<i64> {
        None
    }

    /// Id of the query the update carries, for [`HandlerError::query_id`].
    fn query_id(&self) -> Option<i64> {
        None
    }

    fn order_key(&self) -> Option<i64> {
        self.chat_id()
    }

    /// Runs once before the handlers get the item: fills what the client's fetch options ask for.
    fn prepare(self) -> impl Future<Output = Self> + Send {
        std::future::ready(self)
    }
}

impl Routed for Message {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }

    fn message_id(&self) -> Option<i32> {
        Some(self.id)
    }

    fn user_id(&self) -> Option<i64> {
        self.from_user.as_ref().map(|user| user.id)
    }

    fn prepare(self) -> impl Future<Output = Self> + Send {
        prefetched(self)
    }
}

async fn prefetched(mut message: Message) -> Message {
    if !message.client().0.fetch.any() {
        return message;
    }
    let client = message.client().clone();
    client.prefetch(std::slice::from_mut(&mut message)).await;
    message
}

impl Routed for CallbackQuery {
    fn chat_id(&self) -> Option<i64> {
        self.chat().map(|c| c.id)
    }

    // `CallbackQuery`'s own accessor, which wins over this method of the same name.
    fn message_id(&self) -> Option<i32> {
        CallbackQuery::message_id(self)
    }

    fn user_id(&self) -> Option<i64> {
        Some(self.from_user.id)
    }

    fn query_id(&self) -> Option<i64> {
        Some(self.id)
    }

    fn order_key(&self) -> Option<i64> {
        Some(self.chat().map_or(self.from_user.id, |c| c.id))
    }
}

impl Routed for WidgetQuery {
    fn chat_id(&self) -> Option<i64> {
        self.chat().map(|c| c.id)
    }

    // `WidgetQuery`'s own accessor, which wins over this method of the same name.
    fn message_id(&self) -> Option<i32> {
        WidgetQuery::message_id(self)
    }

    fn user_id(&self) -> Option<i64> {
        Some(self.from_user.id)
    }

    fn query_id(&self) -> Option<i64> {
        Some(self.id)
    }

    fn order_key(&self) -> Option<i64> {
        Some(self.chat().map_or(self.from_user.id, |c| c.id))
    }
}

impl Routed for SharedWidgetQuery {
    fn chat_id(&self) -> Option<i64> {
        None
    }

    fn user_id(&self) -> Option<i64> {
        Some(self.from_user.id)
    }

    fn query_id(&self) -> Option<i64> {
        Some(self.id)
    }

    fn order_key(&self) -> Option<i64> {
        Some(self.from_user.id)
    }
}

impl Routed for InlineQuery {
    fn chat_id(&self) -> Option<i64> {
        None
    }

    fn user_id(&self) -> Option<i64> {
        Some(self.from_user.id)
    }

    fn query_id(&self) -> Option<i64> {
        Some(self.id)
    }

    fn order_key(&self) -> Option<i64> {
        Some(self.from_user.id)
    }
}

impl Routed for ChosenInlineResult {
    fn chat_id(&self) -> Option<i64> {
        None
    }

    fn user_id(&self) -> Option<i64> {
        Some(self.from_user.id)
    }

    fn order_key(&self) -> Option<i64> {
        Some(self.from_user.id)
    }
}

impl Routed for ChatJoinRequest {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }

    fn user_id(&self) -> Option<i64> {
        Some(self.from_user.id)
    }

    fn query_id(&self) -> Option<i64> {
        self.query_id
    }
}

impl Routed for GuestMessage {
    fn chat_id(&self) -> Option<i64> {
        Some(self.message.chat.id)
    }

    fn message_id(&self) -> Option<i32> {
        Some(self.message.id)
    }

    fn user_id(&self) -> Option<i64> {
        // The sender of the message that invoked the bot, or who it came via when that message names no sender.
        self.message.from_user.as_deref().or(self.caller_user.as_ref()).map(|user| user.id)
    }

    fn query_id(&self) -> Option<i64> {
        Some(self.query_id)
    }
}

impl Routed for BotSubscriptionUpdated {
    fn chat_id(&self) -> Option<i64> {
        Some(self.user.id)
    }

    fn user_id(&self) -> Option<i64> {
        Some(self.user.id)
    }
}

impl Routed for MessageGenerationStopped {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }
}

impl Routed for WidgetDataUpdated {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }

    fn message_id(&self) -> Option<i32> {
        Some(self.message_id)
    }
}

impl Routed for ManagedBotUpdated {
    fn chat_id(&self) -> Option<i64> {
        None
    }

    fn user_id(&self) -> Option<i64> {
        Some(self.user.id)
    }

    fn order_key(&self) -> Option<i64> {
        Some(self.bot.id)
    }
}

impl Routed for EphemeralMessage {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }

    fn message_id(&self) -> Option<i32> {
        Some(self.id)
    }

    fn user_id(&self) -> Option<i64> {
        self.from_user.as_ref().map(|user| user.id)
    }
}

impl Routed for MessageReactionUpdated {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }

    fn message_id(&self) -> Option<i32> {
        Some(self.message_id)
    }

    fn user_id(&self) -> Option<i64> {
        self.user.as_ref().map(|user| user.id)
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

    fn message_id(&self) -> Option<i32> {
        Some(self.message_id)
    }
}

impl Routed for ReactionsUpdated {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }

    fn message_id(&self) -> Option<i32> {
        Some(self.message_id)
    }
}

impl Routed for BusinessConnection {
    fn chat_id(&self) -> Option<i64> {
        Some(self.user.id)
    }

    fn user_id(&self) -> Option<i64> {
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

    fn user_id(&self) -> Option<i64> {
        Some(self.from_user.id)
    }

    fn query_id(&self) -> Option<i64> {
        Some(self.id)
    }
}

impl Routed for PreCheckoutQuery {
    fn chat_id(&self) -> Option<i64> {
        Some(self.from_user.id)
    }

    fn user_id(&self) -> Option<i64> {
        Some(self.from_user.id)
    }

    fn query_id(&self) -> Option<i64> {
        Some(self.id)
    }
}

impl Routed for PurchasedPaidMedia {
    fn chat_id(&self) -> Option<i64> {
        Some(self.from_user.id)
    }

    fn user_id(&self) -> Option<i64> {
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

    fn user_id(&self) -> Option<i64> {
        Some(self.from_user.id)
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

    fn user_id(&self) -> Option<i64> {
        self.user.as_ref().map(|user| user.id)
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

impl Routed for PinnedMessagesUpdated {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }
}

impl Routed for UserStatusUpdated {
    fn chat_id(&self) -> Option<i64> {
        None
    }

    fn user_id(&self) -> Option<i64> {
        Some(self.user.id)
    }

    fn order_key(&self) -> Option<i64> {
        Some(self.user.id)
    }
}

impl Routed for TonWalletState {
    fn chat_id(&self) -> Option<i64> {
        None
    }
}

impl Routed for ChatActionUpdated {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }

    fn user_id(&self) -> Option<i64> {
        self.from_user.as_ref().map(|user| user.id)
    }
}

impl Routed for ReadHistoryUpdated {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }
}

impl Routed for PendingJoinRequestsUpdated {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }
}

impl Routed for StarsBalanceUpdated {
    fn chat_id(&self) -> Option<i64> {
        None
    }
}

impl Routed for Story {
    fn chat_id(&self) -> Option<i64> {
        Some(self.chat.id)
    }

    fn user_id(&self) -> Option<i64> {
        self.from_user.as_ref().map(|user| user.id)
    }
}
