use std::any::{Any, TypeId, type_name};
use std::collections::HashMap;
use std::fmt;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;
use std::sync::Arc;

use super::{UpdateContext, UpdateKind};
use crate::Client;
use crate::errors::{Error, Result};

/// Values provided to handlers, one per type: by [`Client::provide`], by [`Router::provide`](super::Router::provide)
/// for the handlers inside a router, and by middleware with
/// [`UpdateContext::provide`]. A router's value wins over the client's, and a middleware's over both. Cloning it is
/// cheap: the values are shared.
#[derive(Clone, Default)]
pub struct Data(HashMap<TypeId, Arc<dyn Any + Send + Sync>>);

impl Data {
    /// Adds `value`, replacing the value of the same type.
    pub fn insert<V: Send + Sync + 'static>(&mut self, value: V) {
        self.0.insert(TypeId::of::<V>(), Arc::new(value));
    }

    /// The value of type `V`, if one was provided.
    pub fn get<V: 'static>(&self) -> Option<&V> {
        self.0.get(&TypeId::of::<V>()).and_then(|value| (**value).downcast_ref::<V>())
    }

    /// Whether a value of type `V` was provided.
    pub fn contains<V: 'static>(&self) -> bool {
        self.0.contains_key(&TypeId::of::<V>())
    }

    /// Adds every value of `other`, which win over values of the same types here.
    pub(crate) fn extend(&mut self, other: &Data) {
        self.0.extend(other.0.iter().map(|(id, value)| (*id, value.clone())));
    }
}

impl fmt::Debug for Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Data").field("values", &self.0.len()).finish()
    }
}

/// A handler argument holding a clone of the provided value of type `V`. The value is cloned for every update, so
/// provide large or shared state as an `Arc`. A handler asking for a value nobody provided fails with
/// [`MissingDependency`] instead of running; take `Option<Dep<V>>` for a value that may be missing.
///
/// ```no_run
/// use std::sync::Arc;
///
/// use vaegram::handlers::{Dep, Router};
/// use vaegram::prelude::*;
///
/// struct Db {
///     greeting: String,
/// }
///
/// async fn start(msg: Message, Dep(db): Dep<Arc<Db>>) -> Result<()> {
///     msg.reply(db.greeting.as_str()).await?;
///     Ok(())
/// }
///
/// # fn f(client: &Client) {
/// client.provide(Arc::new(Db { greeting: "Hi".into() }));
/// let mut router = Router::new();
/// router.message().handle(filters::command("start"), start);
/// client.include_router(router);
/// # }
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Dep<V>(pub V);

impl<V> Deref for Dep<V> {
    type Target = V;

    fn deref(&self) -> &V {
        &self.0
    }
}

impl<V> DerefMut for Dep<V> {
    fn deref_mut(&mut self) -> &mut V {
        &mut self.0
    }
}

/// Why a handler didn't run: it takes a [`Dep<V>`] for a type nobody provided. It reaches `on_error` inside
/// `Error::Other`; read it back with `error.downcast_ref::<MissingDependency>()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct MissingDependency {
    /// The name of the missing type, as [`std::any::type_name`] gives it.
    pub type_name: &'static str,
}

impl fmt::Display for MissingDependency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a handler takes a `{}` nobody provided: provide it on the client, the handler's router or one around \
             it, or in a middleware before the handler",
            self.type_name
        )
    }
}

impl std::error::Error for MissingDependency {}

/// A handler argument taken from the update's context, after the update itself: [`Dep<V>`], `Option<Dep<V>>`,
/// [`Client`], [`UpdateKind`] and [`Data`]. Implement it for an argument of your own, built from what a middleware
/// provided or from the update: the context still holds the update while the arguments are taken, so
/// [`UpdateContext::event`] and [`UpdateContext::message`] work here.
#[diagnostic::on_unimplemented(
    message = "`{Self}` can't be a handler argument",
    label = "not taken from the update's context",
    note = "after the update, a handler takes `Dep<V>`, `Option<Dep<V>>`, `Client`, `UpdateKind`, `Data`, or a type \
            that implements `FromContext`"
)]
pub trait FromContext: Sized {
    /// Takes the argument from `ctx`; an `Err` keeps the handler from running and goes to `on_error`.
    fn from_context(ctx: &UpdateContext) -> Result<Self>;
}

impl<V: Clone + Send + Sync + 'static> FromContext for Dep<V> {
    fn from_context(ctx: &UpdateContext) -> Result<Self> {
        let missing = || Error::Other(Box::new(MissingDependency { type_name: type_name::<V>() }));
        ctx.data().get::<V>().cloned().map(Dep).ok_or_else(missing)
    }
}

impl<V: Clone + Send + Sync + 'static> FromContext for Option<Dep<V>> {
    fn from_context(ctx: &UpdateContext) -> Result<Self> {
        Ok(ctx.data().get::<V>().cloned().map(Dep))
    }
}

impl FromContext for Client {
    fn from_context(ctx: &UpdateContext) -> Result<Self> {
        Ok(ctx.client().clone())
    }
}

impl FromContext for UpdateKind {
    fn from_context(ctx: &UpdateContext) -> Result<Self> {
        Ok(ctx.kind())
    }
}

impl FromContext for Data {
    fn from_context(ctx: &UpdateContext) -> Result<Self> {
        Ok(ctx.data().clone())
    }
}

/// A handler for [`Observer::handle`](super::Observer::handle): an async function or closure that takes the update,
/// then up to 8 [`FromContext`] arguments, and returns `Result<_>`. Closures need their argument types written out;
/// async functions don't.
#[diagnostic::on_unimplemented(
    message = "`{Self}` can't handle these updates",
    label = "not a handler for this update kind",
    note = "a handler takes the update first, then up to 8 arguments such as `Dep<V>`, and returns \
            `vaegram::Result<_>`; write out a closure's argument types"
)]
pub trait Handler<T, Args>: Send + Sync + 'static {
    /// Takes the other arguments from `ctx`, then the update itself, and starts the handler. A missing argument fails
    /// the call without running the handler.
    fn call(&self, ctx: UpdateContext) -> Pin<Box<dyn Future<Output = Result<()>> + Send>>;
}

macro_rules! handler_arity {
    ($($arg:ident $value:ident),*) => {
        impl<F, Fut, R, T, $($arg,)*> Handler<T, ($($arg,)*)> for F
        where
            F: Fn(T, $($arg,)*) -> Fut + Send + Sync + 'static,
            Fut: Future<Output = Result<R>> + Send + 'static,
            T: 'static,
            $($arg: FromContext,)*
        {
            fn call(&self, mut ctx: UpdateContext) -> Pin<Box<dyn Future<Output = Result<()>> + Send>> {
                // The arguments come first, while the context still holds the update for them to read.
                $(
                    let $value = match <$arg as FromContext>::from_context(&ctx) {
                        Ok(value) => value,
                        Err(error) => return Box::pin(std::future::ready(Err(error))),
                    };
                )*
                let item = ctx.take::<T>();
                let future = (self)(item, $($value,)*);
                Box::pin(async move { future.await.map(drop) })
            }
        }
    };
}

handler_arity!();
handler_arity!(A1 a1);
handler_arity!(A1 a1, A2 a2);
handler_arity!(A1 a1, A2 a2, A3 a3);
handler_arity!(A1 a1, A2 a2, A3 a3, A4 a4);
handler_arity!(A1 a1, A2 a2, A3 a3, A4 a4, A5 a5);
handler_arity!(A1 a1, A2 a2, A3 a3, A4 a4, A5 a5, A6 a6);
handler_arity!(A1 a1, A2 a2, A3 a3, A4 a4, A5 a5, A6 a6, A7 a7);
handler_arity!(A1 a1, A2 a2, A3 a3, A4 a4, A5 a5, A6 a6, A7 a7, A8 a8);
