use std::ops::{BitAnd, BitOr, Not};
use std::sync::Arc;

use crate::enums::ChatType;
use crate::raw::base;
use crate::types::{CallbackQuery, Chat, InlineQuery, Message, User};

/// Updates with a chat, for `private()`, `group()`, `channel()` and `chat(id)`.
pub trait HasChat {
    fn chat(&self) -> &Chat;
}

/// Updates with a sender, for `bot()` and `user(id)`.
pub trait HasSender {
    fn sender(&self) -> Option<&User>;
}

/// Updates with text, for `text()`.
pub trait HasText {
    fn text(&self) -> Option<&str>;
}

/// Updates that can carry a bot command, for `command()` and `commands()`.
pub trait HasCommand {
    fn command(&self) -> Option<Vec<&str>>;
}

impl HasChat for Message {
    fn chat(&self) -> &Chat {
        &self.chat
    }
}

impl HasSender for Message {
    fn sender(&self) -> Option<&User> {
        self.from_user.as_ref()
    }
}

impl HasText for Message {
    fn text(&self) -> Option<&str> {
        let plain = matches!(self.raw.media, None | Some(base::MessageMedia::MessageMediaWebPage(_)));
        (plain && !self.text.is_empty()).then_some(&self.text)
    }
}

impl HasCommand for Message {
    fn command(&self) -> Option<Vec<&str>> {
        Message::command(self)
    }
}

impl HasChat for CallbackQuery {
    fn chat(&self) -> &Chat {
        &self.chat
    }
}

impl HasSender for CallbackQuery {
    fn sender(&self) -> Option<&User> {
        Some(&self.from_user)
    }
}

impl HasSender for InlineQuery {
    fn sender(&self) -> Option<&User> {
        Some(&self.from_user)
    }
}

impl HasText for InlineQuery {
    fn text(&self) -> Option<&str> {
        Some(self.query.as_str()).filter(|q| !q.is_empty())
    }
}

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

pub fn text<T: HasText>() -> Filter<T> {
    create(|m: &T| m.text().is_some())
}

fn chat_kind<T: HasChat>(kinds: &'static [ChatType]) -> Filter<T> {
    create(move |m: &T| kinds.contains(&m.chat().kind))
}

pub fn private<T: HasChat>() -> Filter<T> {
    chat_kind(&[ChatType::Private, ChatType::Bot])
}

pub fn group<T: HasChat>() -> Filter<T> {
    chat_kind(&[ChatType::Group, ChatType::Supergroup])
}

pub fn channel<T: HasChat>() -> Filter<T> {
    chat_kind(&[ChatType::Channel])
}

pub fn incoming() -> Filter {
    create(|m: &Message| !m.outgoing)
}

pub fn outgoing() -> Filter {
    create(|m: &Message| m.outgoing)
}

pub fn bot<T: HasSender>() -> Filter<T> {
    create(|m: &T| m.sender().is_some_and(|u| u.is_bot))
}

pub fn user<T: HasSender>(id: i64) -> Filter<T> {
    create(move |m: &T| m.sender().is_some_and(|u| u.id == id))
}

pub fn chat<T: HasChat>(id: i64) -> Filter<T> {
    create(move |m: &T| m.chat().id == id)
}

pub fn command<T: HasCommand>(name: &str) -> Filter<T> {
    commands([name])
}

pub fn commands<T: HasCommand, S: Into<String>>(names: impl IntoIterator<Item = S>) -> Filter<T> {
    let names: Vec<String> = names.into_iter().map(Into::into).collect();
    create(move |m: &T| m.command().is_some_and(|c| names.iter().any(|n| n.eq_ignore_ascii_case(c[0]))))
}
