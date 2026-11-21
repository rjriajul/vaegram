use std::sync::Arc;

use super::facts::Facts;
use crate::types::Command;

pub(crate) enum Pred<T> {
    All,
    Flags {
        set: u32,
        clear: u32,
    },
    Media(u32),
    Service(u128),
    ChatKinds(u8),
    Chats(Box<[i64]>),
    Users(Box<[i64]>),
    Commands {
        prefixes: Prefixes,
        names: Box<[Box<str>]>,
        case_sensitive: bool,
    },
    TextPrefix(Box<str>),
    Data(Box<[u8]>),
    DataPrefix(Box<[u8]>),
    Thread(i32),
    #[cfg(feature = "regex")]
    Regex(::regex::Regex),
    Custom(Arc<dyn Fn(&T) -> bool + Send + Sync>),
    And(Box<[Pred<T>]>),
    Or(Box<[Pred<T>]>),
    Not(Box<Pred<T>>),
}

/// Where a clone stands in a group it is copying.
enum Copying<'a, T> {
    Not,
    List { and: bool, rest: std::slice::Iter<'a, Pred<T>>, done: Vec<Pred<T>> },
}

/// Opens `list` for copying: the first child to copy next, or the finished copy of an empty list.
fn open_list<'a, T>(open: &mut Vec<Copying<'a, T>>, and: bool, list: &'a [Pred<T>]) -> Result<&'a Pred<T>, Pred<T>> {
    let mut rest = list.iter();
    match rest.next() {
        Some(first) => {
            open.push(Copying::List { and, rest, done: Vec::with_capacity(list.len()) });
            Ok(first)
        }
        None if and => Err(Pred::And(Box::default())),
        None => Err(Pred::Or(Box::default())),
    }
}

/// Hands a finished copy to the group it belongs to, closing every group that this completes. Returns the next
/// child to copy, or the copy of the whole filter once nothing is left open.
fn close_copy<'a, T>(open: &mut Vec<Copying<'a, T>>, mut built: Pred<T>) -> Result<&'a Pred<T>, Pred<T>> {
    loop {
        match open.pop() {
            None => return Err(built),
            Some(Copying::Not) => built = Pred::Not(Box::new(built)),
            Some(Copying::List { and, mut rest, mut done }) => {
                done.push(built);
                match rest.next() {
                    Some(next) => {
                        open.push(Copying::List { and, rest, done });
                        return Ok(next);
                    }
                    None if and => built = Pred::And(done.into()),
                    None => built = Pred::Or(done.into()),
                }
            }
        }
    }
}

/// Copies with an explicit stack, not recursion: a filter nested thousands of levels deep must not overflow the
/// call stack.
impl<T> Clone for Pred<T> {
    fn clone(&self) -> Self {
        let mut open = Vec::new();
        let mut next = self;
        loop {
            let built = match next {
                Pred::Not(p) => {
                    open.push(Copying::Not);
                    next = &**p;
                    continue;
                }
                Pred::And(p) => match open_list(&mut open, true, p) {
                    Ok(first) => {
                        next = first;
                        continue;
                    }
                    Err(empty) => empty,
                },
                Pred::Or(p) => match open_list(&mut open, false, p) {
                    Ok(first) => {
                        next = first;
                        continue;
                    }
                    Err(empty) => empty,
                },
                Pred::All => Pred::All,
                &Pred::Flags { set, clear } => Pred::Flags { set, clear },
                &Pred::Media(m) => Pred::Media(m),
                &Pred::Service(s) => Pred::Service(s),
                &Pred::ChatKinds(c) => Pred::ChatKinds(c),
                Pred::Chats(ids) => Pred::Chats(ids.clone()),
                Pred::Users(ids) => Pred::Users(ids.clone()),
                Pred::Commands { prefixes, names, case_sensitive } => {
                    Pred::Commands { prefixes: prefixes.clone(), names: names.clone(), case_sensitive: *case_sensitive }
                }
                Pred::TextPrefix(p) => Pred::TextPrefix(p.clone()),
                Pred::Data(d) => Pred::Data(d.clone()),
                Pred::DataPrefix(d) => Pred::DataPrefix(d.clone()),
                &Pred::Thread(t) => Pred::Thread(t),
                #[cfg(feature = "regex")]
                Pred::Regex(re) => Pred::Regex(re.clone()),
                Pred::Custom(f) => Pred::Custom(f.clone()),
            };
            match close_copy(&mut open, built) {
                Ok(first) => next = first,
                Err(whole) => return whole,
            }
        }
    }
}

/// Drops nested groups with an explicit stack, not recursion, for the same reason as `clone`.
impl<T> Drop for Pred<T> {
    fn drop(&mut self) {
        let mut groups = Vec::new();
        self.take_groups(&mut groups);
        while let Some(mut group) = groups.pop() {
            group.take_groups(&mut groups);
        }
    }
}

/// How deep `walk` recurses before it carries on with an explicit stack.
const WALK_DEPTH: usize = 32;

/// Where a walk stands in a group it is evaluating.
enum Walking<'a, T> {
    Not,
    List { stop: bool, rest: std::slice::Iter<'a, Pred<T>>, known: bool },
}

impl<T> Pred<T> {
    /// Whether `item` passes, with every closure run.
    pub(crate) fn eval(&self, f: &Facts<'_>, item: &T) -> bool {
        self.walk(f, Some(item), 0).unwrap_or(false)
    }

    /// The verdict from the facts alone: `None` while a closure that needs the parsed update decides it.
    pub(crate) fn decide(&self, f: &Facts<'_>) -> Option<bool> {
        self.walk(f, None, 0)
    }

    /// A leaf's verdict, and a closure's when there is an item to run it on. `None` for a group, which `walk` takes
    /// apart before it asks.
    fn atom(&self, f: &Facts<'_>, item: Option<&T>) -> Option<bool> {
        Some(match self {
            Pred::All => true,
            &Pred::Flags { set, clear } => f.flags & set == set && f.flags & clear == 0,
            &Pred::Media(mask) => f.media.is_some_and(|k| mask & (1 << k as u8) != 0),
            &Pred::Service(mask) => f.service.is_some_and(|k| mask & (1 << k as u8) != 0),
            &Pred::ChatKinds(mask) => f.chat.is_some_and(|c| mask & (1 << c as u8) != 0),
            Pred::Chats(ids) => ids.binary_search(&f.chat_id).is_ok(),
            Pred::Users(ids) => ids.binary_search(&f.sender_id).is_ok(),
            Pred::Commands { prefixes, names, case_sensitive } => prefixes.parse(f.text).is_some_and(|c| {
                c.is_for(f.me)
                    && names.iter().any(|n| if *case_sensitive { **n == *c.name } else { same_text(n, c.name) })
            }),
            Pred::TextPrefix(p) => f.text.starts_with(&**p),
            Pred::Data(d) => f.data == &**d,
            Pred::DataPrefix(d) => f.data.starts_with(d),
            &Pred::Thread(t) => f.thread_id == t,
            #[cfg(feature = "regex")]
            Pred::Regex(re) => super::regex::seen(f).is_some_and(|text| re.is_match(text)),
            Pred::Custom(c) => return item.map(|i| c(i)),
            Pred::And(_) | Pred::Or(_) | Pred::Not(_) => return None,
        })
    }

    /// The verdict, three-valued: a closure with no `item` to run on is unknown, and a group is settled by the first
    /// child that decides it, known or not. Recurses `WALK_DEPTH` levels, then carries on with `walk_deep`.
    fn walk(&self, f: &Facts<'_>, item: Option<&T>, depth: usize) -> Option<bool> {
        if depth >= WALK_DEPTH {
            return self.walk_deep(f, item);
        }
        let (stop, list) = match self {
            Pred::Not(p) => return p.walk(f, item, depth + 1).map(|b| !b),
            Pred::And(p) => (false, p),
            Pred::Or(p) => (true, p),
            leaf => return leaf.atom(f, item),
        };
        let mut known = true;
        for p in list.iter() {
            match p.walk(f, item, depth + 1) {
                Some(b) if b == stop => return Some(stop),
                Some(_) => {}
                None => known = false,
            }
        }
        known.then_some(!stop)
    }

    /// `walk` with an explicit stack, so a filter nested thousands of levels deep can't overflow the call stack.
    fn walk_deep(&self, f: &Facts<'_>, item: Option<&T>) -> Option<bool> {
        let mut open: Vec<Walking<'_, T>> = Vec::new();
        let mut next = self;
        loop {
            let group = match next {
                Pred::Not(p) => {
                    open.push(Walking::Not);
                    next = &**p;
                    continue;
                }
                Pred::And(p) => Some((false, p)),
                Pred::Or(p) => Some((true, p)),
                _ => None,
            };
            let mut value = match group {
                None => next.atom(f, item),
                Some((stop, list)) => {
                    let mut rest = list.iter();
                    match rest.next() {
                        Some(first) => {
                            open.push(Walking::List { stop, rest, known: true });
                            next = first;
                            continue;
                        }
                        None => Some(!stop),
                    }
                }
            };
            loop {
                match open.pop() {
                    None => return value,
                    Some(Walking::Not) => value = value.map(|b| !b),
                    Some(Walking::List { stop, mut rest, mut known }) => {
                        match value {
                            Some(b) if b == stop => continue,
                            Some(_) => {}
                            None => known = false,
                        }
                        match rest.next() {
                            Some(first) => {
                                open.push(Walking::List { stop, rest, known });
                                next = first;
                                break;
                            }
                            None => value = known.then_some(!stop),
                        }
                    }
                }
            }
        }
    }

    /// Moves this group's nested groups out into `out`, leaving it empty, so that none is dropped inside another.
    fn take_groups(&mut self, out: &mut Vec<Pred<T>>) {
        match self {
            Pred::And(p) | Pred::Or(p) => {
                for child in std::mem::take(p).into_vec() {
                    if matches!(child, Pred::And(_) | Pred::Or(_) | Pred::Not(_)) {
                        out.push(child);
                    }
                }
            }
            Pred::Not(p) => {
                if matches!(**p, Pred::And(_) | Pred::Or(_) | Pred::Not(_)) {
                    out.push(std::mem::replace(&mut **p, Pred::All));
                }
            }
            _ => {}
        }
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

    fn into_and(mut self) -> Vec<Pred<T>> {
        if let Pred::And(p) = &mut self {
            return std::mem::take(p).into_vec();
        }
        vec![self]
    }

    fn into_or(mut self) -> Vec<Pred<T>> {
        if let Pred::Or(p) = &mut self {
            return std::mem::take(p).into_vec();
        }
        vec![self]
    }
}

/// What starts a command: any one character of a set, or any one of several strings.
#[derive(Clone)]
pub(crate) enum Prefixes {
    Chars(Box<str>),
    Strings(Box<[Box<str>]>),
}

impl Prefixes {
    fn parse<'a>(&self, text: &'a str) -> Option<Command<'a>> {
        match self {
            Prefixes::Chars(set) => Command::parse(text, set),
            Prefixes::Strings(list) => Command::parse_prefixed(text, list),
        }
    }
}

/// Whether the two are equal once lowercased, in any script. Works char by char, so it allocates nothing.
fn same_text(a: &str, b: &str) -> bool {
    a.chars().flat_map(char::to_lowercase).eq(b.chars().flat_map(char::to_lowercase))
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEPTH: usize = 100_000;

    fn facts() -> Facts<'static> {
        Facts { flags: 1, thread_id: 5, ..Facts::default() }
    }

    /// `DEPTH` groups around `inner`, cycling through `!`, `& thread 5` and `| flag 2`. The facts make the last two
    /// pass and fail the way that keeps the verdict, so it only flips at each `!`. Returns the filter and its
    /// verdict, given `inner`'s.
    fn nested(inner: Pred<()>, mut verdict: bool) -> (Pred<()>, bool) {
        let mut p = inner;
        for i in 0..DEPTH {
            p = match i % 3 {
                0 => {
                    verdict = !verdict;
                    Pred::Not(Box::new(p))
                }
                1 => p.and(Pred::Thread(5)),
                _ => p.or(Pred::Flags { set: 2, clear: 0 }),
            };
        }
        (p, verdict)
    }

    #[test]
    fn a_deeply_nested_filter_evaluates_clones_and_drops_without_overflowing_the_stack() {
        let (p, verdict) = nested(Pred::Flags { set: 1, clear: 0 }, true);
        assert_eq!(p.decide(&facts()), Some(verdict));
        assert_eq!(p.eval(&facts(), &()), verdict);
        let copy = p.clone();
        drop(p);
        assert_eq!(copy.decide(&facts()), Some(verdict));
        assert_eq!(copy.eval(&facts(), &()), verdict);
    }

    #[test]
    fn a_closure_deep_inside_is_unknown_until_there_is_an_item() {
        let (p, verdict) = nested(Pred::Custom(Arc::new(|_: &()| true)), true);
        assert_eq!(p.decide(&facts()), None);
        assert_eq!(p.eval(&facts(), &()), verdict);
    }

    #[test]
    fn a_clone_keeps_every_group() {
        let p: Pred<()> = Pred::And(Box::new([Pred::Or(Box::default()), Pred::Not(Box::new(Pred::Thread(5)))]));
        let copy = p.clone();
        assert!(matches!(&copy, Pred::And(l) if l.len() == 2
            && matches!(&l[0], Pred::Or(e) if e.is_empty())
            && matches!(&l[1], Pred::Not(n) if matches!(**n, Pred::Thread(5)))));
        assert_eq!(copy.decide(&facts()), Some(false));
        let empty: Pred<()> = Pred::And(Box::default());
        assert_eq!(empty.clone().decide(&facts()), Some(true));
    }
}
