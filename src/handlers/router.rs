use std::fmt;

use super::middleware::{Next, UpdateContext, middleware};
use super::{Endpoint, Handler, HandlerError, HandlerId, Registered, UpdateKind, error_callback};
use crate::dispatcher::Router as Table;
use crate::dispatcher::router::{Handlers, Target, next_id};
use crate::errors::Result;
use crate::filters::Filter;

/// A set of handlers to include in a client, or in another router: one module of a bot with its own middleware,
/// values and error handler.
///
/// Register handlers with the same `on_*` methods the [`Client`](crate::Client) has, then include the router with
/// [`Client::include_router`](crate::Client::include_router), or nest it with [`include_router`](Self::include_router).
/// An included router is one entry in its parent's groups, like a handler: when one of its handlers matches, it has
/// taken its group's turn; its own handlers run by their groups inside it. Its handlers keep their ids, so
/// [`Client::remove_handler`](crate::Client::remove_handler) and [`Client::handlers`](crate::Client::handlers) reach
/// them.
///
/// Filters still decide before the update is parsed, through every level, so a router whose filters miss costs
/// nothing. An [`outer_middleware`](Self::outer_middleware) needs the parsed update, and so does a handler from
/// [`Observer::handle`]: an update that reaches one is parsed and walked in its handler task from there on, and
/// [`on_unhandled_update`](crate::Client::on_unhandled_update) gets it there when nothing took it. Listeners from
/// `ask` and `listen` still get messages and button presses before any router.
///
/// ```no_run
/// use vaegram::handlers::{Next, Router, UpdateContext};
/// use vaegram::prelude::*;
///
/// # fn f(client: &Client) {
/// let mut admin = Router::new();
/// admin.message().filter(filters::private());
/// admin.outer_middleware(|ctx: UpdateContext, next: Next| async move {
///     if ctx.user_id() == Some(42) { next.run(ctx).await } else { Ok(()) }
/// });
/// admin.on_message(filters::command("ban"), |msg| async move { msg.reply("Banned").await });
/// client.include_router(admin);
/// # }
/// ```
#[derive(Default)]
pub struct Router(pub(crate) Table);

impl Router {
    /// An empty router.
    pub fn new() -> Self {
        Router::default()
    }

    /// Includes `router` in this one, as one more entry in its groups: group 0 unless the returned
    /// [`RouteRef`] says otherwise. This router's middleware and values apply to it.
    pub fn include_router(&mut self, router: Router) -> RouteRef<'_> {
        let id = next_id();
        self.0.mount(id, router.0);
        RouteRef { router: &mut self.0, id: Registered::Router(id) }
    }

    /// Adds a middleware that runs on every update reaching this router, before its filters, for the kinds this
    /// router has handlers for (an [`Observer::outer_middleware`] adds one kind). It gets the parsed update and a
    /// [`Next`]:
    ///
    /// - running `next` hands the update to this router's filters and handlers, and gives back their result;
    /// - returning without running it keeps the update: the routes after this router in its group don't get it;
    /// - when it runs `next` and nothing here matched, the routes after this router in its group get the update as it
    ///   arrived, without this middleware's changes.
    ///
    /// Middleware runs in the order it was added, the first one outermost; a parent router's runs before its
    /// children's.
    pub fn outer_middleware<F, Fut>(&mut self, f: F) -> &mut Self
    where
        F: Fn(UpdateContext, Next) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.0.layer.outer.push((None, middleware(f)));
        self
    }

    /// Adds a middleware that runs around each handler of this router, and of the routers included in it, whose
    /// filter matched, after its outer middleware. Returning without running `next` skips that handler, which still
    /// takes its group's turn. A parent router's inner middleware runs outside its children's.
    pub fn inner_middleware<F, Fut>(&mut self, f: F) -> &mut Self
    where
        F: Fn(UpdateContext, Next) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.0.layer.inner.push((None, middleware(f)));
        self
    }

    /// Provides `value` to the middleware and handlers of this router and of the routers in it, which take it as a
    /// [`Dep<V>`](super::Dep). It wins over a value of the same type the client or an outer router provides, and
    /// replaces one provided here before.
    pub fn provide<V: Send + Sync + 'static>(&mut self, value: V) -> &mut Self {
        self.0.layer.data.insert(value);
        self
    }

    /// Receives every `Err` from this router's handlers and middleware, and from the routers in it that have no
    /// `on_error` of their own. Like an `Err` reaching the client, it stops the later groups for that update, here
    /// and outside. Without it, the error goes up to the outer router's `on_error`, and last to
    /// [`Client::on_error`](crate::Client::on_error). Registering again replaces it.
    pub fn on_error<F, Fut>(&mut self, f: F) -> &mut Self
    where
        F: Fn(HandlerError) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.0.on_error = Some(error_callback(f));
        self
    }
}

impl fmt::Debug for Router {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Router").finish_non_exhaustive()
    }
}

/// A handler just registered on a [`Router`], or a router just included in one: set its group within that router,
/// make it stop later groups, or take its [`HandlerId`]. Dropping it keeps the handler registered.
pub struct RouteRef<'a> {
    router: &'a mut Table,
    id: Registered,
}

// `group` and `stop` take effect at once; `self` comes back only for chaining, so dropping it is fine.
#[allow(clippy::return_self_not_must_use)]
impl RouteRef<'_> {
    /// Groups run in ascending order, default 0, within the router; the first match in a group wins.
    pub fn group(self, group: i32) -> Self {
        self.router.configure_registered(self.id, Some(group), false);
        self
    }

    /// When this handler matches, or this included router takes the update, the router's groups after it don't run
    /// for that update.
    pub fn stop(self) -> Self {
        self.router.configure_registered(self.id, None, true);
        self
    }

    /// An id to pass to `Client::remove_handler` once the router is included in a client.
    pub fn id(self) -> HandlerId {
        HandlerId(self.id)
    }
}

impl fmt::Debug for RouteRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RouteRef").field("id", &self.id).finish_non_exhaustive()
    }
}

/// One kind of update in a [`Router`], from methods such as [`Router::message`]: handlers that take values from the
/// update's context, and a filter or middleware for this kind alone.
pub struct Observer<'a, T> {
    router: &'a mut Table,
    kind: UpdateKind,
    list: fn(&mut Table) -> &mut Handlers<T>,
}

impl<'a, T: Send + 'static> Observer<'a, T> {
    pub(crate) fn new(router: &'a mut Table, kind: UpdateKind, list: fn(&mut Table) -> &mut Handlers<T>) -> Self {
        Observer { router, kind, list }
    }

    pub(crate) fn route(self, filter: Filter<T>, target: Target<T>) -> RouteRef<'a> {
        let id = next_id();
        (self.list)(&mut *self.router).add(id, filter, target);
        RouteRef { router: self.router, id: Registered::Route(self.kind, id) }
    }

    /// Registers a handler that takes, after the update, arguments from its context: provided values as
    /// [`Dep<V>`](super::Dep), the [`Client`](crate::Client), and anything that implements
    /// [`FromContext`](super::FromContext). It runs for the updates that pass `filter` and reach this router.
    ///
    /// ```no_run
    /// use vaegram::handlers::{Dep, Router};
    /// use vaegram::prelude::*;
    ///
    /// #[derive(Clone)]
    /// struct Greeting(&'static str);
    ///
    /// async fn start(msg: Message, Dep(greeting): Dep<Greeting>) -> Result<()> {
    ///     msg.reply(greeting.0).await?;
    ///     Ok(())
    /// }
    ///
    /// let mut router = Router::new();
    /// router.provide(Greeting("Hi"));
    /// router.message().handle(filters::command("start"), start);
    /// ```
    pub fn handle<H, Args>(&mut self, filter: Filter<T>, handler: H) -> RouteRef<'_>
    where
        H: Handler<T, Args>,
        Args: 'static,
    {
        let endpoint: Endpoint = std::sync::Arc::new(move |ctx: UpdateContext| handler.call(ctx));
        let observer = Observer { router: &mut *self.router, kind: self.kind, list: self.list };
        observer.route(filter, Target::Handle(endpoint))
    }

    /// Adds a filter every update of this kind must pass before any of this router's handlers, or the routers in it,
    /// see it, and before its outer middleware runs. Built-in filters decide before the update is parsed. Adding
    /// another keeps both.
    pub fn filter(&mut self, filter: Filter<T>) -> &mut Self {
        let list = (self.list)(&mut *self.router);
        list.filter = Some(match list.filter.take() {
            Some(old) => old & filter,
            None => filter,
        });
        self
    }

    /// Adds an outer middleware, as [`Router::outer_middleware`] does, for this kind alone. The router then takes
    /// part in this kind even without a handler for it: the middleware sees every such update reaching the router.
    pub fn outer_middleware<F, Fut>(&mut self, f: F) -> &mut Self
    where
        F: Fn(UpdateContext, Next) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.router.layer.outer.push((Some(self.kind), middleware(f)));
        self
    }

    /// Adds an inner middleware, as [`Router::inner_middleware`] does, for this kind alone.
    pub fn inner_middleware<F, Fut>(&mut self, f: F) -> &mut Self
    where
        F: Fn(UpdateContext, Next) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<()>> + Send + 'static,
    {
        self.router.layer.inner.push((Some(self.kind), middleware(f)));
        self
    }
}

impl<T> fmt::Debug for Observer<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Observer").field("kind", &self.kind).finish_non_exhaustive()
    }
}
