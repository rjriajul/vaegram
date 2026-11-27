//! The part of routing that runs in an update's task: included routers, their middleware, and the handlers that
//! take values from the update's context. The walk before the task stops at the first of these an update reaches;
//! the rest of that kind's routes, later groups included, are walked here, on the routes as they were then.

use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use super::router::{Handlers, Mount, Target};
use super::tasks::guard;
use crate::Client;
use crate::errors::Result;
use crate::filters::{Filter, Inspect, Subject};
use crate::handlers::{
    BoxFuture, Callback, Data, Endpoint, HandlerError, Mw, Routed, Terminal, UpdateContext, UpdateKind, chain,
};
use crate::session::Events;

/// What a router, or the rest of a list, did with an update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Flow {
    /// Nothing matched, or an outer middleware let the update through and nothing inside matched: the routes after
    /// it in the same group get it.
    Unhandled,
    /// A handler ran, or an outer middleware kept the update: the group is taken.
    Handled,
    /// An error a router's `on_error` took: like an `Err`, it ends the update's walk.
    Stop,
}

type Walk = Pin<Box<dyn Future<Output = Result<Flow>> + Send>>;

/// A router's middleware and provided values; the client's router keeps only values. A middleware without a kind
/// runs for every kind the router is included for.
#[derive(Default, Clone)]
pub(crate) struct Layer {
    pub(crate) outer: Vec<(Option<UpdateKind>, Mw)>,
    pub(crate) inner: Vec<(Option<UpdateKind>, Mw)>,
    pub(crate) data: Data,
}

impl Layer {
    /// Whether an outer middleware runs for `kind`.
    pub(crate) fn outer(&self, kind: UpdateKind) -> bool {
        self.outer.iter().any(|(scope, _)| applies(*scope, kind))
    }

    /// Whether an outer middleware was added for `kind` alone, which makes the router take part in that kind even
    /// without a route for it.
    pub(crate) fn scoped_outer(&self, kind: UpdateKind) -> bool {
        self.outer.iter().any(|(scope, _)| *scope == Some(kind))
    }
}

fn applies(scope: Option<UpdateKind>, kind: UpdateKind) -> bool {
    scope.is_none_or(|scope| scope == kind)
}

fn for_kind(list: &[(Option<UpdateKind>, Mw)], kind: UpdateKind) -> impl Iterator<Item = Mw> + '_ {
    list.iter().filter(move |(scope, _)| applies(*scope, kind)).map(|(_, mw)| mw.clone())
}

/// Where a walk stands: the values provided so far, and the inner middleware of the routers it is inside, outermost
/// first.
#[derive(Clone)]
struct Scope {
    client: Client,
    kind: UpdateKind,
    data: Data,
    inner: Vec<Mw>,
    events: Arc<Events>,
}

impl Scope {
    /// Tests `filter` on the parsed update; a panic in it counts as no match, as before the task.
    fn test<T: Routed + Subject + Inspect>(&self, filter: &Filter<T>, item: &T) -> bool {
        let me = self.client.0.me.lock().unwrap().clone();
        let facts = item.facts(&me);
        guard(&self.events, self.kind, item.chat_id(), || filter.eval(&facts, item)).unwrap_or(false)
    }

    fn context<T: Routed>(&self, item: T) -> UpdateContext {
        UpdateContext::new(self.client.clone(), self.kind, item, self.data.clone())
    }

    /// Runs a matched handler inside the inner middleware.
    fn run<T: Routed>(&self, item: T, terminal: Terminal) -> BoxFuture {
        chain(&self.inner, self.context(item), terminal)
    }
}

/// Walks `handlers` from route `at` on, `group` already taken, in the update's task: what `Handlers::finish` left.
pub(crate) async fn resume<T: Routed + Subject + Inspect>(
    handlers: Handlers<T>,
    kind: UpdateKind,
    at: usize,
    group: Option<i32>,
    item: T,
) -> Result<Flow> {
    let client = Inspect::client(&item).clone();
    // The client's values, as they are now: `provide` doesn't wait for a new router to be taken.
    let data = client.0.handlers.current().layer.data.clone();
    let events = client.0.events.clone();
    let scope = Scope { client, kind, data, inner: Vec::new(), events };
    walk(handlers, at, group, item, scope).await
}

/// The same walk as before the task: groups in order, the first match of each, `stop` ending it; an `Err` ends it
/// too and goes up.
fn walk<T: Routed + Subject + Inspect>(
    handlers: Handlers<T>,
    start: usize,
    mut group: Option<i32>,
    item: T,
    scope: Scope,
) -> Walk {
    Box::pin(async move {
        let mut flow = Flow::Unhandled;
        for route in handlers.routes.iter().skip(start) {
            if group == Some(route.group) {
                continue;
            }
            let step = match &route.target {
                Target::Router(mount) => enter(mount, item.clone(), &scope).await?,
                Target::Call(callback) => {
                    if !scope.test(&route.filter, &item) {
                        continue;
                    }
                    if scope.inner.is_empty() {
                        callback(item.clone()).await?;
                    } else {
                        scope.run(item.clone(), plain(callback.clone())).await?;
                    }
                    Flow::Handled
                }
                Target::Handle(endpoint) => {
                    if !scope.test(&route.filter, &item) {
                        continue;
                    }
                    scope.run(item.clone(), taking(endpoint.clone())).await?;
                    Flow::Handled
                }
            };
            match step {
                Flow::Unhandled => continue,
                Flow::Stop => return Ok(Flow::Stop),
                Flow::Handled => flow = Flow::Handled,
            }
            if route.stop {
                break;
            }
            group = Some(route.group);
        }
        Ok(flow)
    })
}

/// Hands the update to an included router: its filter, then its outer middleware around its routes. An `Err` from
/// inside goes to the router's `on_error` when it has one, and ends the walk; otherwise it goes up.
fn enter<T: Routed + Subject + Inspect>(mount: &Mount<T>, item: T, parent: &Scope) -> Walk {
    let (node, handlers) = (&mount.node, mount.handlers().clone());
    let kind = parent.kind;
    let mut scope = parent.clone();
    scope.data.extend(&node.layer.data);
    scope.inner.extend(for_kind(&node.layer.inner, kind));
    let outer: Vec<Mw> = for_kind(&node.layer.outer, kind).collect();
    let on_error = node.on_error.clone();
    Box::pin(async move {
        if handlers.filter.as_ref().is_some_and(|filter| !scope.test(filter, &item)) {
            return Ok(Flow::Unhandled);
        }
        let (chat_id, message_id) = (item.chat_id(), item.message_id());
        let (user_id, query_id) = (item.user_id(), item.query_id());
        let flow = if outer.is_empty() {
            walk(handlers, 0, None, item, scope).await
        } else {
            around(&outer, handlers, item, scope).await
        };
        match (flow, on_error) {
            (Err(error), Some(on_error)) => {
                on_error(HandlerError { kind, chat_id, message_id, user_id, query_id, error }).await;
                Ok(Flow::Stop)
            }
            (flow, _) => flow,
        }
    })
}

// What the routes inside an outer middleware did, for the router to report once the middleware returns.
const KEPT: u8 = 0;
const PASSED: u8 = 1;
const HANDLED: u8 = 2;
const STOPPED: u8 = 3;

/// Runs a router's outer middleware around its routes. A middleware that returns without running `next` keeps the
/// update; one that runs it when nothing inside matched lets it through.
async fn around<T: Routed + Subject + Inspect>(
    outer: &[Mw],
    handlers: Handlers<T>,
    item: T,
    scope: Scope,
) -> Result<Flow> {
    let state = Arc::new(AtomicU8::new(KEPT));
    let seen = state.clone();
    let context = scope.context(item);
    let terminal: Terminal = Box::new(move |mut ctx: UpdateContext| -> BoxFuture {
        let item = ctx.take::<T>();
        // What the middleware provided reaches the routes inside.
        let scope = Scope { data: ctx.into_data(), ..scope };
        seen.store(PASSED, Ordering::Relaxed);
        Box::pin(async move {
            let flow = walk(handlers, 0, None, item, scope).await;
            let state = match &flow {
                Ok(Flow::Unhandled) => PASSED,
                Ok(Flow::Stop) => STOPPED,
                // A middleware that catches an `Err` from inside has dealt with the update.
                Ok(Flow::Handled) | Err(_) => HANDLED,
            };
            seen.store(state, Ordering::Relaxed);
            flow.map(drop)
        })
    });
    chain(outer, context, terminal).await?;
    Ok(match state.load(Ordering::Relaxed) {
        PASSED => Flow::Unhandled,
        STOPPED => Flow::Stop,
        _ => Flow::Handled,
    })
}

/// The end of a middleware chain for a `Router::on_*` handler.
fn plain<T: Send + 'static>(callback: Callback<T>) -> Terminal {
    Box::new(move |mut ctx: UpdateContext| -> BoxFuture { callback(ctx.take::<T>()) })
}

/// The end of a middleware chain for a handler that takes values from the context: it takes them, then the update.
fn taking(endpoint: Endpoint) -> Terminal {
    Box::new(move |ctx: UpdateContext| -> BoxFuture { endpoint(ctx) })
}
