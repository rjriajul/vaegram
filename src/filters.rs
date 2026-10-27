use std::ops::{BitAnd, BitOr, Not};
use std::sync::Arc;

use crate::enums::ChatType;
use crate::raw::base;
use crate::types::{CallbackQuery, Message};

pub struct Filter<T = Message>(Arc<dyn Fn(&T) -> bool + Send + Sync>);

impl<T> Clone for Filter<T> {
    fn clone(&self) -> Self {
        Filter(self.0.clone())
    }
}

impl<T> Filter<T> {
    pub fn check(&self, value: &T) -> bool {
        (self.0)(value)
    }
}

pub fn create<T>(f: impl Fn(&T) -> bool + Send + Sync + 'static) -> Filter<T> {
    Filter(Arc::new(f))
}

impl<T: 'static> BitAnd for Filter<T> {
    type Output = Filter<T>;
    fn bitand(self, other: Filter<T>) -> Filter<T> {
        create(move |v| self.check(v) && other.check(v))
    }
}

impl<T: 'static> BitOr for Filter<T> {
    type Output = Filter<T>;
    fn bitor(self, other: Filter<T>) -> Filter<T> {
        create(move |v| self.check(v) || other.check(v))
    }
}

impl<T: 'static> Not for Filter<T> {
    type Output = Filter<T>;
    fn not(self) -> Filter<T> {
        create(move |v| !self.check(v))
    }
}

pub fn all<T>() -> Filter<T> {
    create(|_| true)
}

pub fn data(value: impl Into<String>) -> Filter<CallbackQuery> {
    let value = value.into();
    create(move |q: &CallbackQuery| q.data == value)
}

pub fn text() -> Filter {
    create(|m: &Message| {
        !m.text.is_empty() && matches!(m.raw.media, None | Some(base::MessageMedia::MessageMediaWebPage(_)))
    })
}

pub fn private() -> Filter {
    create(|m: &Message| matches!(m.chat.kind, ChatType::Private | ChatType::Bot))
}

pub fn group() -> Filter {
    create(|m: &Message| matches!(m.chat.kind, ChatType::Group | ChatType::Supergroup))
}

pub fn channel() -> Filter {
    create(|m: &Message| m.chat.kind == ChatType::Channel)
}

pub fn incoming() -> Filter {
    create(|m: &Message| !m.outgoing)
}

pub fn outgoing() -> Filter {
    create(|m: &Message| m.outgoing)
}

pub fn bot() -> Filter {
    create(|m: &Message| m.from_user.as_ref().is_some_and(|u| u.is_bot))
}

pub fn user(id: i64) -> Filter {
    create(move |m: &Message| m.from_user.as_ref().is_some_and(|u| u.id == id))
}

pub fn chat(id: i64) -> Filter {
    create(move |m: &Message| m.chat.id == id)
}

pub fn command(name: &str) -> Filter {
    commands([name])
}

pub fn commands<S: Into<String>>(names: impl IntoIterator<Item = S>) -> Filter {
    let names: Vec<String> = names.into_iter().map(Into::into).collect();
    create(move |m: &Message| m.command().is_some_and(|c| names.iter().any(|n| n.eq_ignore_ascii_case(c[0]))))
}
