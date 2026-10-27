use std::sync::Arc;

use super::facts::Facts;
use crate::types::Command;

pub(crate) enum Pred<T> {
    All,
    Flags { set: u32, clear: u32 },
    Media(u32),
    Service(u64),
    ChatKinds(u8),
    Chats(Box<[i64]>),
    Users(Box<[i64]>),
    Commands { prefixes: Box<str>, names: Box<[Box<str>]> },
    TextPrefix(Box<str>),
    Data(Box<[u8]>),
    DataPrefix(Box<[u8]>),
    Thread(i32),
    Custom(Arc<dyn Fn(&T) -> bool + Send + Sync>),
    And(Box<[Pred<T>]>),
    Or(Box<[Pred<T>]>),
    Not(Box<Pred<T>>),
}

impl<T> Clone for Pred<T> {
    fn clone(&self) -> Self {
        match self {
            Pred::All => Pred::All,
            &Pred::Flags { set, clear } => Pred::Flags { set, clear },
            &Pred::Media(m) => Pred::Media(m),
            &Pred::Service(s) => Pred::Service(s),
            &Pred::ChatKinds(c) => Pred::ChatKinds(c),
            Pred::Chats(ids) => Pred::Chats(ids.clone()),
            Pred::Users(ids) => Pred::Users(ids.clone()),
            Pred::Commands { prefixes, names } => Pred::Commands { prefixes: prefixes.clone(), names: names.clone() },
            Pred::TextPrefix(p) => Pred::TextPrefix(p.clone()),
            Pred::Data(d) => Pred::Data(d.clone()),
            Pred::DataPrefix(d) => Pred::DataPrefix(d.clone()),
            &Pred::Thread(t) => Pred::Thread(t),
            Pred::Custom(f) => Pred::Custom(f.clone()),
            Pred::And(p) => Pred::And(p.clone()),
            Pred::Or(p) => Pred::Or(p.clone()),
            Pred::Not(p) => Pred::Not(p.clone()),
        }
    }
}

impl<T> Pred<T> {
    pub(crate) fn eval(&self, f: &Facts<'_>, item: Option<&T>) -> bool {
        match self {
            Pred::All => true,
            &Pred::Flags { set, clear } => f.flags & set == set && f.flags & clear == 0,
            &Pred::Media(mask) => f.media.is_some_and(|k| mask & (1 << k as u8) != 0),
            &Pred::Service(mask) => f.service.is_some_and(|k| mask & (1 << k as u8) != 0),
            &Pred::ChatKinds(mask) => f.chat.is_some_and(|c| mask & (1 << c as u8) != 0),
            Pred::Chats(ids) => ids.binary_search(&f.chat_id).is_ok(),
            Pred::Users(ids) => ids.binary_search(&f.sender_id).is_ok(),
            Pred::Commands { prefixes, names } => Command::parse(f.text, prefixes)
                .is_some_and(|c| c.is_for(f.me) && names.iter().any(|n| n.eq_ignore_ascii_case(c.name))),
            Pred::TextPrefix(p) => f.text.starts_with(&**p),
            Pred::Data(d) => f.data == &**d,
            Pred::DataPrefix(d) => f.data.starts_with(d),
            &Pred::Thread(t) => f.thread_id == t,
            Pred::Custom(c) => item.is_some_and(|i| c(i)),
            Pred::And(p) => p.iter().all(|p| p.eval(f, item)),
            Pred::Or(p) => p.iter().any(|p| p.eval(f, item)),
            Pred::Not(p) => !p.eval(f, item),
        }
    }

    pub(crate) fn decide(&self, f: &Facts<'_>) -> Option<bool> {
        let (stop, list) = match self {
            Pred::Custom(_) => return None,
            Pred::Not(p) => return p.decide(f).map(|b| !b),
            Pred::And(p) => (false, p),
            Pred::Or(p) => (true, p),
            p => return Some(p.eval(f, None)),
        };
        let mut known = true;
        for p in list {
            match p.decide(f) {
                Some(b) if b == stop => return Some(stop),
                Some(_) => {}
                None => known = false,
            }
        }
        known.then_some(!stop)
    }

    pub(crate) fn and(self, other: Pred<T>) -> Pred<T> {
        let mut out: Vec<Pred<T>> = Vec::new();
        for p in self.into_and().into_iter().chain(other.into_and()) {
            match (out.last_mut(), &p) {
                (_, Pred::All) => {}
                (Some(Pred::Flags { set, clear }), &Pred::Flags { set: s, clear: c }) => {
                    (*set, *clear) = (*set | s, *clear | c)
                }
                _ => out.push(p),
            }
        }
        match out.len() {
            0 => Pred::All,
            1 => out.pop().unwrap(),
            _ => Pred::And(out.into()),
        }
    }

    pub(crate) fn or(self, other: Pred<T>) -> Pred<T> {
        let mut out: Vec<Pred<T>> = Vec::new();
        for p in self.into_or().into_iter().chain(other.into_or()) {
            match (out.last_mut(), &p) {
                (Some(Pred::Media(a)), &Pred::Media(b)) => *a |= b,
                (Some(Pred::Service(a)), &Pred::Service(b)) => *a |= b,
                (Some(Pred::ChatKinds(a)), &Pred::ChatKinds(b)) => *a |= b,
                _ => out.push(p),
            }
        }
        match out.len() {
            1 => out.pop().unwrap(),
            _ => Pred::Or(out.into()),
        }
    }

    fn into_and(self) -> Vec<Pred<T>> {
        match self {
            Pred::And(p) => p.into_vec(),
            p => vec![p],
        }
    }

    fn into_or(self) -> Vec<Pred<T>> {
        match self {
            Pred::Or(p) => p.into_vec(),
            p => vec![p],
        }
    }
}
