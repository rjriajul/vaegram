use std::any::Any;
use std::fmt;
use std::sync::Arc;

use super::{BoxFuture, Data, Routed, UpdateKind};
use crate::Client;
use crate::errors::Result;
use crate::types::{CallbackQuery, Message};

/// A middleware as a router keeps it.
pub(crate) type Mw = Arc<dyn Fn(UpdateContext, Next) -> BoxFuture + Send + Sync>;
/// What the last [`Next`] of a chain runs: the router's routes, or the matched handler.
pub(crate) type Terminal = Box<dyn FnOnce(UpdateContext) -> BoxFuture + Send>;

/// Turns a middleware closure into the form a router keeps.
pub(crate) fn middleware<F, Fut>(f: F) -> Mw
where
    F: Fn(UpdateContext, Next) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    Arc::new(move |ctx: UpdateContext, next: Next| -> BoxFuture { Box::pin(f(ctx, next)) })
}

/// Runs `ctx` through `middleware`, the first one outermost, then `terminal`.
pub(crate) fn chain(middleware: &[Mw], ctx: UpdateContext, terminal: Terminal) -> BoxFuture {
    let mut next = Next(terminal);
    for mw in middleware.iter().rev() {
        let (mw, inner) = (mw.clone(), next);
        next = Next(Box::new(move |ctx: UpdateContext| -> BoxFuture { mw(ctx, inner) }));
    }
    (next.0)(ctx)
}

/// One update on its way through a router's middleware: the event, the client that received it, and the values
/// provided for its handlers. A middleware gets it with a [`Next`], changes it if it likes, and hands it on with
/// [`Next::run`], or returns without doing so to keep the update from the handlers.
///
/// ```no_run
/// use vaegram::handlers::{Next, Router, UpdateContext};
///
/// #[derive(Clone)]
/// struct Admin(i64);
///
/// let mut router = Router::new();
/// router.outer_middleware(|mut ctx: UpdateContext, next: Next| async move {
///     match ctx.user_id() {
///         Some(id) if id == 42 => {
///             ctx.provide(Admin(id));
///             next.run(ctx).await
///         }
///         // Kept: neither this router's handlers nor the routers after it in its group see the update.
///         _ => Ok(()),
///     }
/// });
/// ```
pub struct UpdateContext {
    client: Client,
    kind: UpdateKind,
    chat_id: Option<i64>,
    user_id: Option<i64>,
    event: Box<dyn Any + Send + Sync>,
    data: Data,
}

impl UpdateContext {
    pub(crate) fn new<T: Routed>(client: Client, kind: UpdateKind, item: T, data: Data) -> Self {
        let (chat_id, user_id) = (item.chat_id(), item.user_id());
        UpdateContext { client, kind, chat_id, user_id, event: Box::new(item), data }
    }

    /// The client that received the update.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// The route the update is on, which says what [`event`](Self::event) holds: a [`Message`] for the four message
    /// kinds, a [`CallbackQuery`] for [`UpdateKind::CallbackQuery`], and so on.
    pub fn kind(&self) -> UpdateKind {
        self.kind
    }

    /// Bot-API style id of the update's chat, as [`HandlerError::chat_id`](super::HandlerError::chat_id) has it.
    pub fn chat_id(&self) -> Option<i64> {
        self.chat_id
    }

    /// Bot-API style id of the user who caused the update, as
    /// [`HandlerError::user_id`](super::HandlerError::user_id) has it.
    pub fn user_id(&self) -> Option<i64> {
        self.user_id
    }

    /// The update's event, when it is a `T`.
    pub fn event<T: 'static>(&self) -> Option<&T> {
        (*self.event).downcast_ref::<T>()
    }

    /// The update's event, when it is a `T`, to change before the handlers get it. The change stays inside this
    /// router: when nothing in it matches, the routers after it get the update as it arrived.
    pub fn event_mut<T: 'static>(&mut self) -> Option<&mut T> {
        (*self.event).downcast_mut::<T>()
    }

    /// The message, for the new, edited and business message kinds.
    pub fn message(&self) -> Option<&Message> {
        self.event()
    }

    /// The button press, for [`UpdateKind::CallbackQuery`].
    pub fn callback_query(&self) -> Option<&CallbackQuery> {
        self.event()
    }

    /// The values provided so far: the client's, the routers' on the way here, and the middleware's before.
    pub fn data(&self) -> &Data {
        &self.data
    }

    /// The values provided so far, to change for the middleware and handlers after this one.
    pub fn data_mut(&mut self) -> &mut Data {
        &mut self.data
    }

    /// Provides `value` to the middleware and handlers after this one, which take it as a
    /// [`Dep<V>`](super::Dep). It replaces a value of the same type provided before.
    pub fn provide<V: Send + Sync + 'static>(&mut self, value: V) {
        self.data.insert(value);
    }

    /// Takes the event out, as the `T` the context was made with.
    pub(crate) fn take<T: 'static>(&mut self) -> T {
        let event = std::mem::replace(&mut self.event, Box::new(()));
        // Only `new` sets the event, and the routes of one kind all hold the same type.
        *event.downcast::<T>().unwrap_or_else(|_| unreachable!("an update context holds the event it was made with"))
    }

    pub(crate) fn into_data(self) -> Data {
        self.data
    }
}

impl fmt::Debug for UpdateContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UpdateContext")
            .field("kind", &self.kind)
            .field("chat_id", &self.chat_id)
            .field("user_id", &self.user_id)
            .field("data", &self.data)
            .finish_non_exhaustive()
    }
}

/// The rest of the chain after a middleware: the middleware after it, then the router's handlers. [`run`](Self::run)
/// takes it by value, so it runs at most once.
pub struct Next(pub(crate) Terminal);

impl Next {
    /// Hands `ctx` on and waits for the rest of the chain. Its `Err` is a handler's, or a later middleware's: return
    /// it to pass it up, or handle it here.
    pub async fn run(self, ctx: UpdateContext) -> Result<()> {
        (self.0)(ctx).await
    }
}

impl fmt::Debug for Next {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Next").finish_non_exhaustive()
    }
}
