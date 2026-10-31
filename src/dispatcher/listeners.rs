use std::mem;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard};

use tokio::sync::oneshot;
use tokio::time::Instant;

use crate::client::Me;
use crate::errors::{Error, Result};
use crate::filters::{Filter, Inspect, Subject};
use crate::types::{CallbackQuery, InlineMessageId, Message};

static UNEXPECTED_CLICK: LazyLock<Arc<str>> = LazyLock::new(|| "You are not expected to click this button.".into());

/// An update kind `Client::listen` can wait for: [`Message`] or [`CallbackQuery`].
pub trait Listenable: Listened {}

pub trait Listened: Subject + Inspect + Sized {
    fn book(listeners: &Listeners) -> &Book<Self>;
    fn identity<'a>(&'a self, me: &Me) -> Option<Identity<'a>>;
}

/// What a listener's criteria are checked against.
pub struct Identity<'a> {
    pub(crate) chat: Option<i64>,
    pub(crate) user: Option<i64>,
    pub(crate) message: Option<i32>,
    pub(crate) inline: Option<&'a InlineMessageId>,
}

impl Listenable for Message {}

impl Listened for Message {
    fn book(listeners: &Listeners) -> &Book<Message> {
        &listeners.messages
    }

    fn identity<'a>(&'a self, me: &Me) -> Option<Identity<'a>> {
        // Your own messages answer only in Saved Messages, where you talk to yourself.
        if self.outgoing && self.chat.id != me.id {
            return None;
        }
        let user = self.from_user.as_ref().map(|u| u.id).or(self.sender_chat.as_ref().map(|c| c.id));
        Some(Identity { chat: Some(self.chat.id), user, message: Some(self.id), inline: None })
    }
}

impl Listenable for CallbackQuery {}

impl Listened for CallbackQuery {
    fn book(listeners: &Listeners) -> &Book<CallbackQuery> {
        &listeners.callbacks
    }

    fn identity<'a>(&'a self, _: &Me) -> Option<Identity<'a>> {
        Some(Identity {
            chat: self.chat().map(|c| c.id),
            user: Some(self.from_user.id),
            message: self.message_id(),
            inline: self.inline_message_id(),
        })
    }
}

#[derive(Default)]
pub struct Listeners {
    pub(crate) messages: Book<Message>,
    pub(crate) callbacks: Book<CallbackQuery>,
    next_id: AtomicU64,
}

impl Listeners {
    pub(crate) fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub(crate) fn open(&self) {
        self.messages.open();
        self.callbacks.open();
    }

    /// Ends every listener with `Error::ListenerStopped` and refuses new ones until `open`.
    pub(crate) fn close(&self) {
        self.messages.close();
        self.callbacks.close();
    }
}

/// Every empty list matches anything; a filled one must contain the update's value.
pub(crate) struct Criteria<T> {
    pub(crate) chats: Vec<i64>,
    pub(crate) users: Vec<i64>,
    pub(crate) messages: Vec<i32>,
    pub(crate) inline: Vec<InlineMessageId>,
    pub(crate) after: Option<i32>,
    // Shared, so `offer` can copy it out of the lock without cloning the filter tree.
    pub(crate) filter: Option<Arc<Filter<T>>>,
    pub(crate) deadline: Option<Instant>,
    pub(crate) alert: Option<Arc<str>>,
}

impl<T> Default for Criteria<T> {
    fn default() -> Self {
        Criteria {
            chats: Vec::new(),
            users: Vec::new(),
            messages: Vec::new(),
            inline: Vec::new(),
            after: None,
            filter: None,
            deadline: None,
            alert: Some(UNEXPECTED_CLICK.clone()),
        }
    }
}

impl<T> Criteria<T> {
    fn live(&self, now: Instant) -> bool {
        self.deadline.is_none_or(|deadline| deadline > now)
    }

    /// Everything but the user.
    fn places(&self, id: &Identity<'_>) -> bool {
        fits(&self.chats, id.chat.as_ref())
            && fits(&self.messages, id.message.as_ref())
            && fits(&self.inline, id.inline)
            && self.after.is_none_or(|after| id.message.is_some_and(|m| m > after))
    }

    fn matches(&self, id: &Identity<'_>) -> bool {
        self.places(id) && fits(&self.users, id.user.as_ref())
    }

    /// The alert for someone else pressing a button this listener is pinned to.
    fn stranger(&self, id: &Identity<'_>) -> Option<Arc<str>> {
        let pinned = !self.messages.is_empty() || !self.inline.is_empty();
        let alert = self.alert.as_ref().filter(|_| pinned && !self.users.is_empty())?;
        (self.places(id) && !fits(&self.users, id.user.as_ref())).then(|| alert.clone())
    }
}

fn fits<V: PartialEq>(list: &[V], value: Option<&V>) -> bool {
    list.is_empty() || value.is_some_and(|v| list.contains(v))
}

struct Entry<T> {
    id: u64,
    criteria: Criteria<T>,
    sink: Sink<T>,
}

enum Sink<T> {
    Wait(oneshot::Sender<T>),
}

struct Shelf<T> {
    entries: Vec<Entry<T>>,
    closed: bool,
}

/// The listeners of one update kind, oldest first.
pub struct Book<T> {
    shelf: Mutex<Shelf<T>>,
    // The entry count, so the router skips all listener work with one load when nobody listens.
    waiting: AtomicUsize,
}

impl<T> Default for Book<T> {
    fn default() -> Self {
        Book { shelf: Mutex::new(Shelf { entries: Vec::new(), closed: false }), waiting: AtomicUsize::new(0) }
    }
}

pub(crate) enum Offer<T> {
    Claimed,
    Unclaimed(T),
    /// Nobody took it, but another user pressed a pinned button: answer with this alert.
    Stranger(T, Arc<str>),
}

impl<T> Book<T> {
    pub(crate) fn waiting(&self) -> bool {
        self.waiting.load(Ordering::Acquire) != 0
    }

    fn shelf(&self) -> MutexGuard<'_, Shelf<T>> {
        // No user code runs under this lock, so it can't be poisoned.
        self.shelf.lock().unwrap()
    }

    fn count(&self, shelf: &Shelf<T>) {
        self.waiting.store(shelf.entries.len(), Ordering::Release);
    }

    fn add(&self, entry: Entry<T>) -> Result<()> {
        let mut shelf = self.shelf();
        if shelf.closed {
            return Err(Error::ListenerStopped);
        }
        shelf.entries.push(entry);
        self.count(&shelf);
        Ok(())
    }

    fn take(&self, id: u64) -> Option<Entry<T>> {
        let mut shelf = self.shelf();
        let at = shelf.entries.iter().position(|e| e.id == id)?;
        let entry = shelf.entries.remove(at);
        self.count(&shelf);
        Some(entry)
    }

    pub(crate) fn remove(&self, id: u64) -> bool {
        self.take(id).is_some()
    }

    fn open(&self) {
        self.shelf().closed = false;
    }

    fn close(&self) {
        let entries = {
            let mut shelf = self.shelf();
            shelf.closed = true;
            self.waiting.store(0, Ordering::Release);
            mem::take(&mut shelf.entries)
        };
        drop(entries);
    }

    fn wait(&self, id: u64, criteria: Criteria<T>) -> Result<oneshot::Receiver<T>> {
        let (tx, rx) = oneshot::channel();
        self.add(Entry { id, criteria, sink: Sink::Wait(tx) })?;
        Ok(rx)
    }

    /// Waits for the update `criteria` describes. Dropping the future removes the listener.
    pub(crate) async fn listen(&self, id: u64, criteria: Criteria<T>) -> Result<T> {
        let deadline = criteria.deadline;
        let mut rx = self.wait(id, criteria)?;
        let _guard = Guard { book: self, id };
        let Some(deadline) = deadline else { return rx.await.map_err(|_| Error::ListenerStopped) };
        match tokio::time::timeout_at(deadline, &mut rx).await {
            Ok(got) => got.map_err(|_| Error::ListenerStopped),
            Err(_) => self.expired(id, rx).await,
        }
    }

    async fn expired(&self, id: u64, rx: oneshot::Receiver<T>) -> Result<T> {
        // The dispatcher can take the entry as the timer fires; then the update is already in `rx`.
        if self.remove(id) {
            return Err(Error::ListenerTimeout);
        }
        rx.await.map_err(|_| Error::ListenerStopped)
    }
}

impl<T: Listened> Book<T> {
    /// Hands `item` to the oldest listener that matches it. Filters run outside the lock.
    pub(crate) fn offer(&self, mut item: T, me: &Me) -> Offer<T> {
        let now = Instant::now();
        let (mut candidates, stranger) = {
            let Some(identity) = item.identity(me) else { return Offer::Unclaimed(item) };
            let shelf = self.shelf();
            let mut live = shelf.entries.iter().filter(|e| e.criteria.live(now));
            let candidates: Vec<(u64, Option<Arc<Filter<T>>>)> = live
                .clone()
                .filter(|e| e.criteria.matches(&identity))
                .map(|e| (e.id, e.criteria.filter.clone()))
                .collect();
            let stranger = live.find_map(|e| e.criteria.stranger(&identity));
            (candidates, stranger)
        };
        if candidates.iter().any(|(_, filter)| filter.is_some()) {
            let facts = item.facts(me);
            candidates.retain(|(_, filter)| filter.as_ref().is_none_or(|f| f.eval(&facts, &item)));
        }
        for (id, _) in candidates {
            let Some(entry) = self.take(id) else { continue };
            match entry.sink {
                Sink::Wait(tx) => match tx.send(item) {
                    Ok(()) => return Offer::Claimed,
                    Err(back) => item = back,
                },
            }
        }
        match stranger {
            Some(alert) => Offer::Stranger(item, alert),
            None => Offer::Unclaimed(item),
        }
    }
}

struct Guard<'a, T> {
    book: &'a Book<T>,
    id: u64,
}

impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.book.remove(self.id);
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::*;
    use crate::Client;
    use crate::dispatcher::Entities;
    use crate::dispatcher::tasks::Tasks;
    use crate::raw::{base, types};
    use crate::utils::{get_channel_id, get_peer};

    fn raw_message(id: i32, chat: i64, from: Option<i64>, out: bool) -> base::Message {
        let mut raw = Message::bare(id, get_peer(chat), 0);
        (raw.from_id, raw.out, raw.message) = (from.map(get_peer), out, "hi".into());
        base::Message::Message(Box::new(raw))
    }

    pub(crate) fn message(client: &Client, id: i32, chat: i64, from: Option<i64>, out: bool) -> Message {
        Message::from_raw(client, raw_message(id, chat, from, out), &Entities::default()).unwrap()
    }

    pub(crate) fn raw_query(user_id: i64, chat: i64, msg_id: i32) -> types::UpdateBotCallbackQuery {
        types::UpdateBotCallbackQuery {
            query_id: 1,
            user_id,
            peer: get_peer(chat),
            msg_id,
            chat_instance: 3,
            data: Some(b"tap".to_vec()),
            game_short_name: None,
        }
    }

    pub(crate) fn query(client: &Client, user_id: i64, chat: i64, msg_id: i32) -> CallbackQuery {
        CallbackQuery::parse(client, &raw_query(user_id, chat, msg_id), &Entities::default())
    }

    pub(crate) fn raw_inline_query(user_id: i64, id: i32) -> types::UpdateInlineBotCallbackQuery {
        let msg_id = types::InputBotInlineMessageID64 { dc_id: 2, owner_id: 5, id, access_hash: 7 };
        types::UpdateInlineBotCallbackQuery {
            query_id: 1,
            user_id,
            msg_id: base::InputBotInlineMessageID::InputBotInlineMessageID64(Box::new(msg_id)),
            chat_instance: 3,
            data: Some(b"tap".to_vec()),
            game_short_name: None,
        }
    }

    pub(crate) fn inline_query(client: &Client, user_id: i64, id: i32) -> CallbackQuery {
        CallbackQuery::parse_inline(client, raw_inline_query(user_id, id), &Entities::default())
    }

    /// Dispatches on `client`'s own router and registry.
    pub(crate) fn dispatch(client: &Client, update: base::Update) {
        let router = client.0.handlers.current();
        let mut tasks = Tasks::new(false, None);
        (tasks.on_error, tasks.events) = (router.on_error.clone(), client.0.events.clone());
        router.dispatch(client, update, &Arc::default(), &mut tasks);
    }

    pub(crate) async fn until_waiting<T>(book: &Book<T>) {
        for _ in 0..1000 {
            if book.waiting() {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("the listener never registered");
    }

    fn me() -> Me {
        Me { id: 5, ..Me::default() }
    }

    fn claimed<T>(offer: Offer<T>) -> bool {
        matches!(offer, Offer::Claimed)
    }

    #[test]
    fn the_oldest_matching_listener_takes_the_update() {
        let (client, book) = (Client::offline(), Book::<Message>::default());
        let chat = |chats| Criteria { chats, ..Criteria::default() };
        let mut first = book.wait(1, chat(vec![7])).unwrap();
        let mut second = book.wait(2, chat(vec![7])).unwrap();
        assert!(claimed(book.offer(message(&client, 1, 7, None, false), &me())));
        assert!(claimed(book.offer(message(&client, 2, 7, None, false), &me())));
        assert_eq!((first.try_recv().unwrap().id, second.try_recv().unwrap().id), (1, 2));
        assert!(!book.waiting());
    }

    #[test]
    fn criteria_match_chat_user_and_message() {
        let client = Client::offline();
        let (channel, other) = (get_channel_id(9), inline_query(&client, 7, 2));
        let msg = |chat, from| message(&client, 10, chat, from, false);
        let (by_7, by_channel) = (msg(-3, Some(7)), msg(-3, Some(channel)));
        let pinned = query(&client, 7, 7, 4);
        let inline = inline_query(&client, 7, 1);
        let id = |c: &CallbackQuery| c.inline_message_id().cloned().unwrap();
        let none = Criteria::<Message>::default;
        let cases: Vec<(Criteria<Message>, &Message, bool)> = vec![
            (none(), &by_7, true),
            (Criteria { chats: vec![-4], ..none() }, &by_7, false),
            (Criteria { chats: vec![-4, -3], ..none() }, &by_7, true),
            (Criteria { users: vec![1, 7], ..none() }, &by_7, true),
            (Criteria { users: vec![1], ..none() }, &by_7, false),
            (Criteria { users: vec![channel], ..none() }, &by_channel, true),
            (Criteria { after: Some(9), ..none() }, &by_7, true),
            (Criteria { after: Some(10), ..none() }, &by_7, false),
        ];
        for (i, (criteria, item, expected)) in cases.into_iter().enumerate() {
            assert_eq!(criteria.matches(&item.identity(&me()).unwrap()), expected, "message case {i}");
        }
        let none = Criteria::<CallbackQuery>::default;
        let cases: Vec<(Criteria<CallbackQuery>, &CallbackQuery, bool)> = vec![
            (Criteria { messages: vec![4], ..none() }, &pinned, true),
            (Criteria { messages: vec![3], ..none() }, &pinned, false),
            (Criteria { messages: vec![4], ..none() }, &inline, false),
            (Criteria { inline: vec![id(&inline)], ..none() }, &inline, true),
            (Criteria { inline: vec![id(&other)], ..none() }, &inline, false),
            (Criteria { chats: vec![7], ..none() }, &inline, false),
        ];
        for (i, (criteria, item, expected)) in cases.into_iter().enumerate() {
            assert_eq!(criteria.matches(&item.identity(&me()).unwrap()), expected, "callback case {i}");
        }
    }

    #[test]
    fn outgoing_messages_answer_only_in_saved_messages() {
        let client = Client::offline();
        assert!(message(&client, 1, 7, None, true).identity(&me()).is_none());
        let saved = message(&client, 1, 5, None, true);
        assert_eq!(saved.identity(&me()).map(|i| (i.chat, i.user)), Some((Some(5), Some(0))));
    }

    #[test]
    fn a_gone_waiter_does_not_swallow_the_update() {
        let (client, book) = (Client::offline(), Book::<Message>::default());
        drop(book.wait(1, Criteria::default()).unwrap());
        let mut alive = book.wait(2, Criteria::default()).unwrap();
        assert!(claimed(book.offer(message(&client, 1, 7, None, false), &me())));
        assert_eq!(alive.try_recv().unwrap().id, 1);
        drop(book.wait(3, Criteria::default()).unwrap());
        assert!(matches!(book.offer(message(&client, 2, 7, None, false), &me()), Offer::Unclaimed(m) if m.id == 2));
    }

    #[tokio::test]
    async fn close_fails_waiters_and_refuses_new_ones_until_open() {
        let (client, listeners) = (Client::offline(), Listeners::default());
        let rx = listeners.messages.wait(1, Criteria::default()).unwrap();
        listeners.close();
        assert!(rx.await.is_err());
        assert!(!listeners.messages.waiting());
        let refused = listeners.callbacks.listen(2, Criteria::default()).await;
        assert!(matches!(refused, Err(Error::ListenerStopped)), "{refused:?}");
        listeners.open();
        let mut rx = listeners.messages.wait(3, Criteria::default()).unwrap();
        assert!(claimed(listeners.messages.offer(message(&client, 1, 7, None, false), &me())));
        assert!(rx.try_recv().is_ok());
    }

    #[tokio::test(start_paused = true)]
    async fn a_huge_timeout_means_no_deadline() {
        let client = Client::offline();
        let book = Arc::new(Book::<Message>::default());
        let deadline = Instant::now().checked_add(Duration::MAX);
        assert!(deadline.is_none());
        let waiter = tokio::spawn({
            let book = book.clone();
            async move { book.listen(1, Criteria { deadline, ..Criteria::default() }).await }
        });
        until_waiting(&book).await;
        tokio::time::advance(Duration::from_secs(10 * 365 * 86_400)).await;
        assert!(claimed(book.offer(message(&client, 1, 7, None, false), &me())));
        assert_eq!(waiter.await.unwrap().unwrap().id, 1);
    }

    #[tokio::test]
    async fn a_timeout_racing_a_delivery_keeps_the_update() {
        let (client, book) = (Client::offline(), Book::<Message>::default());
        let rx = book.wait(1, Criteria::default()).unwrap();
        assert!(claimed(book.offer(message(&client, 1, 7, None, false), &me())));
        assert_eq!(book.expired(1, rx).await.unwrap().id, 1);

        let rx = book.wait(2, Criteria::default()).unwrap();
        assert!(matches!(book.expired(2, rx).await, Err(Error::ListenerTimeout)));
        assert!(!book.waiting());
    }

    #[test]
    fn strangers_are_found_only_for_pinned_messages() {
        let (client, book) = (Client::offline(), Book::<CallbackQuery>::default());
        let _chat_only = book.wait(1, Criteria { chats: vec![7], users: vec![8], ..Criteria::default() }).unwrap();
        assert!(matches!(book.offer(query(&client, 9, 7, 4), &me()), Offer::Unclaimed(_)));

        let pinned = Criteria { messages: vec![4], users: vec![8], ..Criteria::default() };
        let _pinned = book.wait(2, pinned).unwrap();
        assert!(matches!(book.offer(query(&client, 9, 7, 3), &me()), Offer::Unclaimed(_)));
        let offer = book.offer(query(&client, 9, 7, 4), &me());
        assert!(matches!(&offer, Offer::Stranger(q, text) if q.from_user.id == 9 && **text == **UNEXPECTED_CLICK));

        let silent = Criteria { messages: vec![5], users: vec![8], alert: None, ..Criteria::default() };
        let _silent = book.wait(3, silent).unwrap();
        assert!(matches!(book.offer(query(&client, 9, 7, 5), &me()), Offer::Unclaimed(_)));
        assert!(claimed(book.offer(query(&client, 8, 7, 4), &me())));
    }
}
