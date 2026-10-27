use std::ops::{BitAnd, BitOr, Not};
use std::sync::Arc;

use crate::enums::ChatType;
use crate::raw::base;
use crate::types::Message;

#[derive(Clone)]
pub struct Filter(Arc<dyn Fn(&Message) -> bool + Send + Sync>);

impl Filter {
    pub fn check(&self, message: &Message) -> bool {
        (self.0)(message)
    }
}

pub fn create(f: impl Fn(&Message) -> bool + Send + Sync + 'static) -> Filter {
    Filter(Arc::new(f))
}

impl BitAnd for Filter {
    type Output = Filter;
    fn bitand(self, other: Filter) -> Filter {
        create(move |m| self.check(m) && other.check(m))
    }
}

impl BitOr for Filter {
    type Output = Filter;
    fn bitor(self, other: Filter) -> Filter {
        create(move |m| self.check(m) || other.check(m))
    }
}

impl Not for Filter {
    type Output = Filter;
    fn not(self) -> Filter {
        create(move |m| !self.check(m))
    }
}

pub fn all() -> Filter {
    create(|_| true)
}

pub fn text() -> Filter {
    create(|m| !m.text.is_empty() && matches!(m.raw.media, None | Some(base::MessageMedia::MessageMediaWebPage(_))))
}

pub fn private() -> Filter {
    create(|m| matches!(m.chat.kind, ChatType::Private | ChatType::Bot))
}

pub fn group() -> Filter {
    create(|m| matches!(m.chat.kind, ChatType::Group | ChatType::Supergroup))
}

pub fn channel() -> Filter {
    create(|m| m.chat.kind == ChatType::Channel)
}

pub fn incoming() -> Filter {
    create(|m| !m.outgoing)
}

pub fn outgoing() -> Filter {
    create(|m| m.outgoing)
}

pub fn bot() -> Filter {
    create(|m| m.from_user.as_ref().is_some_and(|u| u.is_bot))
}

pub fn user(id: i64) -> Filter {
    create(move |m| m.from_user.as_ref().is_some_and(|u| u.id == id))
}

pub fn chat(id: i64) -> Filter {
    create(move |m| m.chat.id == id)
}

pub fn command(name: &str) -> Filter {
    commands([name])
}

pub fn commands<S: Into<String>>(names: impl IntoIterator<Item = S>) -> Filter {
    let names: Vec<String> = names.into_iter().map(Into::into).collect();
    create(move |m| m.command().is_some_and(|c| names.iter().any(|n| n.eq_ignore_ascii_case(c[0]))))
}
