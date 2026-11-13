use std::mem;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock};

use tokio::sync::{Notify, OwnedSemaphorePermit, Semaphore, oneshot};
use tokio::time::Instant;

use super::tasks::guard;
use crate::client::Me;
use crate::errors::{Error, Result};
use crate::filters::{Filter, Inspect, Subject};
use crate::handlers::{Callback, UpdateKind};
use crate::types::{CallbackQuery, InlineMessageId, Message};

pub(crate) static UNEXPECTED_CLICK: LazyLock<Arc<str>> =
    LazyLock::new(|| "You are not expected to click this button.".into());

/// An update kind `Client::listen` can wait for: [`Message`] or [`CallbackQuery`].
pub trait Listenable: Listened {}

pub trait Listened: Subject + Inspect + Sized {
    /// The route a panic in a listener's filter is reported under.
    const KIND: UpdateKind;
    fn book(listeners: &Listeners) -> &Book<Self>;
    fn identity<'a>(&'a self, me: &Me) -> Option<Identity<'a>>;

    /// Runs once before `Client::listen` hands the answer back: fills what the client's fetch options ask for.
    fn prepare(self) -> impl Future<Output = Self> + Send {
        std::future::ready(self)
    }
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
    const KIND: UpdateKind = UpdateKind::Message;

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

    fn prepare(self) -> impl Future<Output = Self> + Send {
        crate::handlers::Routed::prepare(self)
    }
}

impl Listenable for CallbackQuery {}

impl Listened for CallbackQuery {
    const KIND: UpdateKind = UpdateKind::CallbackQuery;

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

pub struct Listeners {
    pub(crate) messages: Book<Message>,
    pub(crate) callbacks: Book<CallbackQuery>,
    // Shared with both books: told when one of them has an update to give back to the dispatcher.
    returned: Arc<Notify>,
    next_id: AtomicU64,
    // The cap on listeners alive at once, set when the client opens; absent means no cap.
    budget: OnceLock<(usize, Arc<Semaphore>)>,
}

impl Default for Listeners {
    fn default() -> Self {
        let returned = Arc::new(Notify::new());
        Listeners {
            messages: Book::waking(returned.clone()),
            callbacks: Book::waking(returned.clone()),
            returned,
            next_id: AtomicU64::new(0),
            budget: OnceLock::new(),
        }
    }
}

impl Listeners {
    pub(crate) fn next_id(&self) -> u64 {
        self.next_id.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub(crate) fn limit(&self, max: usize) {
        let _ = self.budget.set((max, Arc::new(Semaphore::new(max))));
    }

    /// A slot for one listener, which comes back when the listener's entry is dropped for any reason: claimed,
    /// timed out, stopped, pruned or closed. Expired next-step handlers sit on their shelf until something prunes
    /// it, so a full budget sweeps them out before it refuses.
    pub(crate) fn permit(&self) -> Result<Option<OwnedSemaphorePermit>> {
        let Some((max, slots)) = self.budget.get() else { return Ok(None) };
        if let Ok(permit) = slots.clone().try_acquire_owned() {
            return Ok(Some(permit));
        }
        self.messages.sweep();
        self.callbacks.sweep();
        match slots.clone().try_acquire_owned() {
            Ok(permit) => Ok(Some(permit)),
            Err(_) => Err(Error::ListenerLimitReached(*max)),
        }
    }

    #[cfg(test)]
    pub(crate) fn free_slots(&self) -> Option<usize> {
        self.budget.get().map(|(_, slots)| slots.available_permits())
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

    pub(crate) fn remove(&self, id: u64) -> bool {
        self.messages.remove(id) || self.callbacks.remove(id)
    }

    /// Fires when a listener gave updates back to route again: see [`Book::take_returned`].
    pub(crate) fn returned(&self) -> &Notify {
        &self.returned
    }
}

/// Every empty list matches anything; a filled one must contain the update's value.
pub(crate) struct Criteria<T> {
    pub(crate) chats: Vec<i64>,
    pub(crate) users: Vec<i64>,
    pub(crate) messages: Vec<i32>,
    pub(crate) inline: Vec<InlineMessageId>,
    pub(crate) after: Option<(i64, i32)>,
    // Shared, so `offer` can copy it out of the lock without cloning the filter tree.
    pub(crate) filter: Option<Arc<Filter<T>>>,
    pub(crate) deadline: Option<Instant>,
    pub(crate) alert: Option<Arc<str>>,
    // Held while the listener is on a shelf, so its slot frees whenever the entry goes.
    pub(crate) permit: Option<OwnedSemaphorePermit>,
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
            permit: None,
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
            && self.after.is_none_or(|(chat, after)| id.chat != Some(chat) || id.message.is_some_and(|m| m > after))
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

    /// Whether this stop pattern covers `listener`: each list the pattern fills must share a value with the
    /// listener's, so a listener without that criterion isn't covered.
    pub(crate) fn includes(&self, listener: &Criteria<T>) -> bool {
        overlaps(&self.chats, &listener.chats)
            && overlaps(&self.users, &listener.users)
            && overlaps(&self.messages, &listener.messages)
            && overlaps(&self.inline, &listener.inline)
    }
}

fn fits<V: PartialEq>(list: &[V], value: Option<&V>) -> bool {
    list.is_empty() || value.is_some_and(|v| list.contains(v))
}

fn overlaps<V: PartialEq>(pattern: &[V], listener: &[V]) -> bool {
    pattern.is_empty() || listener.iter().any(|v| pattern.contains(v))
}

struct Entry<T> {
    id: u64,
    criteria: Criteria<T>,
    sink: Sink<T>,
}

enum Sink<T> {
    Wait(oneshot::Sender<T>),
    Call(Callback<T>),
    /// Keeps the updates it matches until [`Book::release`], oldest first and at most [`MAX_HELD`]; the entry stays
    /// on the shelf meanwhile.
    Hold(Vec<T>),
}

/// The most updates one hold keeps; past it the oldest goes back to the dispatcher. A hold lasts one request, but
/// that request can sleep off flood waits for minutes while a busy chat keeps talking.
const MAX_HELD: usize = 64;

struct Shelf<T> {
    entries: Vec<Entry<T>>,
    closed: bool,
}

/// The listeners of one update kind, oldest first.
pub struct Book<T> {
    shelf: Mutex<Shelf<T>>,
    // The entry count, so the router skips all listener work with one load when nobody listens.
    waiting: AtomicUsize,
    // Updates a listener took but never read, because its future was dropped first, or held and then didn't want;
    // the dispatcher routes them again, so they still reach the other listeners and the handlers.
    returned: Mutex<Vec<Returned<T>>>,
    wake: Arc<Notify>,
}

/// An update a listener gave back, to route again.
pub(crate) struct Returned<T> {
    pub(crate) item: T,
    /// Pushed out of a full hold: no hold may take it again, or a full one would take it back and push out the next,
    /// around and around until the hold ends. Every other update goes back to every listener: its listener is gone
    /// or waits only for newer ones.
    pub(crate) evicted: bool,
}

impl<T> Default for Book<T> {
    fn default() -> Self {
        Book::waking(Arc::default())
    }
}

pub(crate) enum Offer<T> {
    Claimed,
    Unclaimed(T),
    /// Nobody took it, but another user pressed a pinned button: answer with this alert.
    Stranger(T, Arc<str>),
    /// A next-step handler took it: run the callback with it.
    Call(Callback<T>, T),
}

impl<T> Book<T> {
    fn waking(wake: Arc<Notify>) -> Book<T> {
        let shelf = Mutex::new(Shelf { entries: Vec::new(), closed: false });
        Book { shelf, waiting: AtomicUsize::new(0), returned: Mutex::new(Vec::new()), wake }
    }

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
        let expired = {
            let mut shelf = self.shelf();
            if shelf.closed {
                return Err(Error::ListenerStopped);
            }
            let expired = prune(&mut shelf, Instant::now());
            shelf.entries.push(entry);
            self.count(&shelf);
            expired
        };
        drop(expired);
        Ok(())
    }

    fn take(&self, id: u64) -> Option<Entry<T>> {
        let mut shelf = self.shelf();
        let at = shelf.entries.iter().position(|e| e.id == id)?;
        let entry = shelf.entries.remove(at);
        self.count(&shelf);
        Some(entry)
    }

    /// Removes the listener; what it held goes back to the dispatcher.
    pub(crate) fn remove(&self, id: u64) -> bool {
        let Some(entry) = self.take(id) else { return false };
        self.discard(entry);
        true
    }

    fn discard(&self, entry: Entry<T>) {
        if let Sink::Hold(held) = entry.sink {
            self.give_back(held, false);
        }
    }

    fn sweep(&self) {
        let expired = {
            let mut shelf = self.shelf();
            let expired = prune(&mut shelf, Instant::now());
            self.count(&shelf);
            expired
        };
        drop(expired);
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
        // Nobody routes them after a stop.
        drop(self.take_returned());
    }

    /// Updates listeners took but never read: their futures were dropped first, or they held them and then didn't
    /// want them (`evicted` when a full hold pushed them out). Handed to the dispatcher to route again, unless the
    /// listeners are closed.
    fn give_back(&self, items: impl IntoIterator<Item = T>, evicted: bool) {
        if self.shelf().closed {
            return;
        }
        // No user code runs under this lock either.
        let mut returned = self.returned.lock().unwrap();
        let before = returned.len();
        returned.extend(items.into_iter().map(|item| Returned { item, evicted }));
        let added = returned.len() > before;
        drop(returned);
        if added {
            self.wake.notify_one();
        }
    }

    /// What listeners gave back, in the order they gave it.
    pub(crate) fn take_returned(&self) -> Vec<Returned<T>> {
        mem::take(&mut *self.returned.lock().unwrap())
    }

    fn wait(&self, id: u64, criteria: Criteria<T>) -> Result<oneshot::Receiver<T>> {
        let (tx, rx) = oneshot::channel();
        self.add(Entry { id, criteria, sink: Sink::Wait(tx) })?;
        Ok(rx)
    }

    pub(crate) fn call(&self, id: u64, criteria: Criteria<T>, callback: Callback<T>) -> Result<()> {
        self.add(Entry { id, criteria, sink: Sink::Call(callback) })
    }

    /// Starts a listener that keeps every update it matches instead of ending with the first, so none is missed
    /// while the caller does something else first; [`release`](Self::release) ends the hold. Dropping the guard
    /// removes the listener and gives back what it held, in the order it arrived.
    pub(crate) fn hold(&self, id: u64, criteria: Criteria<T>) -> Result<Holding<'_, T>> {
        self.add(Entry { id, criteria, sink: Sink::Hold(Vec::new()) })?;
        Ok(Holding { book: self, id })
    }

    /// Stops every listener `pattern` covers and returns how many; waiters fail with `Error::ListenerStopped`.
    pub(crate) fn stop(&self, pattern: &Criteria<T>) -> usize {
        let stopped: Vec<Entry<T>> = {
            let mut shelf = self.shelf();
            let (stopped, kept) =
                mem::take(&mut shelf.entries).into_iter().partition(|e| pattern.includes(&e.criteria));
            shelf.entries = kept;
            self.count(&shelf);
            stopped
        };
        let count = stopped.len();
        for entry in stopped {
            self.discard(entry);
        }
        count
    }

    /// Waits for the update `criteria` describes. Dropping the future removes the listener, and gives back an
    /// update it was already handed.
    pub(crate) async fn listen(&self, id: u64, criteria: Criteria<T>) -> Result<T> {
        let deadline = criteria.deadline;
        let rx = self.wait(id, criteria)?;
        self.receive(id, rx, deadline).await
    }

    /// Waits until `deadline` for what listener `id` is handed through `rx`, as `listen` describes.
    async fn receive(&self, id: u64, rx: oneshot::Receiver<T>, deadline: Option<Instant>) -> Result<T> {
        let mut guard = Guard { book: self, id, rx };
        let Some(deadline) = deadline else { return (&mut guard.rx).await.map_err(|_| Error::ListenerStopped) };
        match tokio::time::timeout_at(deadline, &mut guard.rx).await {
            Ok(got) => got.map_err(|_| Error::ListenerStopped),
            Err(_) => self.expired(id, &mut guard.rx).await,
        }
    }

    async fn expired(&self, id: u64, rx: &mut oneshot::Receiver<T>) -> Result<T> {
        // The dispatcher can take the entry as the timer fires; then the update is already in `rx`.
        if self.remove(id) {
            return Err(Error::ListenerTimeout);
        }
        rx.await.map_err(|_| Error::ListenerStopped)
    }
}

impl<T: Listened> Book<T> {
    /// Ends the hold [`hold`](Self::hold) started on listener `id`: only updates newer than message `after.1` in
    /// chat `after.0` count now. Returns the first held update that still matches, else waits for one until
    /// `deadline`, as `listen` does. Every other held update goes back to the dispatcher, in the order it arrived.
    pub(crate) async fn release(&self, id: u64, after: (i64, i32), deadline: Option<Instant>, me: &Me) -> Result<T> {
        let (tx, rx) = oneshot::channel();
        let (answer, rest) = {
            let mut shelf = self.shelf();
            // Gone when `stop` or `close` ended it first.
            let at = shelf.entries.iter().position(|e| e.id == id).ok_or(Error::ListenerStopped)?;
            let entry = &mut shelf.entries[at];
            let Sink::Hold(held) = &mut entry.sink else { return Err(Error::ListenerStopped) };
            let mut held = mem::take(held);
            (entry.criteria.after, entry.criteria.deadline) = (Some(after), deadline);
            // Filters ran when the updates were held; only `after` is new.
            let first = held.iter().position(|item| item.identity(me).is_some_and(|i| entry.criteria.matches(&i)));
            let answer = match first {
                Some(first) => {
                    let answer = held.remove(first);
                    shelf.entries.remove(at);
                    self.count(&shelf);
                    Some(answer)
                }
                None => {
                    entry.sink = Sink::Wait(tx);
                    None
                }
            };
            (answer, held)
        };
        self.give_back(rest, false);
        match answer {
            Some(answer) => Ok(answer),
            None => self.receive(id, rx, deadline).await,
        }
    }

    /// Hands `item` to the oldest listener that matches it. Filters run outside the lock.
    pub(crate) fn offer(&self, item: T, me: &Me) -> Offer<T> {
        self.offer_to(item, me, true)
    }

    /// [`offer`](Self::offer) for an update a full hold pushed out: no hold takes it. The full one would take it
    /// back and push out the next, around and around until it ends.
    pub(crate) fn offer_evicted(&self, item: T, me: &Me) -> Offer<T> {
        self.offer_to(item, me, false)
    }

    fn offer_to(&self, mut item: T, me: &Me, holds: bool) -> Offer<T> {
        let now = Instant::now();
        let (mut candidates, stranger, expired, chat) = {
            let Some(identity) = item.identity(me) else { return Offer::Unclaimed(item) };
            let mut shelf = self.shelf();
            let expired = prune(&mut shelf, now);
            self.count(&shelf);
            let mut live = shelf.entries.iter().filter(|e| e.criteria.live(now));
            let candidates: Vec<(u64, Option<Arc<Filter<T>>>)> = live
                .clone()
                .filter(|e| (holds || !matches!(e.sink, Sink::Hold(_))) && e.criteria.matches(&identity))
                .map(|e| (e.id, e.criteria.filter.clone()))
                .collect();
            let stranger = live.find_map(|e| e.criteria.stranger(&identity));
            (candidates, stranger, expired, identity.chat)
        };
        drop(expired);
        if candidates.iter().any(|(_, filter)| filter.is_some()) {
            let (facts, events) = (item.facts(me), &item.client().0.events);
            // A panicking filter counts as a miss, as on a handler.
            let test = |f: &Filter<T>| guard(events, T::KIND, chat, || f.eval(&facts, &item)).unwrap_or(false);
            candidates.retain(|(_, filter)| filter.as_deref().is_none_or(test));
        }
        for (id, _) in candidates {
            let entry = {
                let mut shelf = self.shelf();
                let Some(at) = shelf.entries.iter().position(|e| e.id == id) else { continue };
                let entry = &mut shelf.entries[at];
                // The filters ran unlocked, and `release` may have changed the criteria meanwhile.
                let current = entry.criteria.live(now) && item.identity(me).is_some_and(|i| entry.criteria.matches(&i));
                if !current {
                    continue;
                }
                if let Sink::Hold(held) = &mut entry.sink {
                    let oldest = (held.len() >= MAX_HELD).then(|| held.remove(0));
                    held.push(item);
                    drop(shelf);
                    self.give_back(oldest, true);
                    return Offer::Claimed;
                }
                let entry = shelf.entries.remove(at);
                self.count(&shelf);
                entry
            };
            match entry.sink {
                Sink::Wait(tx) => match tx.send(item) {
                    Ok(()) => return Offer::Claimed,
                    Err(back) => item = back,
                },
                Sink::Call(callback) => return Offer::Call(callback, item),
                Sink::Hold(_) => unreachable!("a holding entry stays on the shelf"),
            }
        }
        match stranger {
            Some(alert) => Offer::Stranger(item, alert),
            None => Offer::Unclaimed(item),
        }
    }
}

// Waiters and holders remove themselves when their timer fires or their owner drops them; expired next-step handlers
// have nobody to do that. They are handed back, so the caller drops their callbacks after unlocking.
fn prune<T>(shelf: &mut Shelf<T>, now: Instant) -> Vec<Entry<T>> {
    let keep = |e: &Entry<T>| matches!(e.sink, Sink::Wait(_) | Sink::Hold(_)) || e.criteria.live(now);
    if shelf.entries.iter().all(keep) {
        return Vec::new();
    }
    let (kept, expired) = mem::take(&mut shelf.entries).into_iter().partition(keep);
    shelf.entries = kept;
    expired
}

struct Guard<'a, T> {
    book: &'a Book<T>,
    id: u64,
    rx: oneshot::Receiver<T>,
}

impl<T> Drop for Guard<'_, T> {
    fn drop(&mut self) {
        self.book.remove(self.id);
        // Handed over after the last poll: the update is in `rx`, taken from every other listener and handler.
        if let Ok(item) = self.rx.try_recv() {
            self.book.give_back([item], false);
        }
    }
}

/// A holding listener, removed when this drops unless [`Book::release`] ended the hold first.
pub(crate) struct Holding<'a, T> {
    book: &'a Book<T>,
    id: u64,
}

impl<T> Drop for Holding<'_, T> {
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

    pub(crate) fn new_message(id: i32, chat: i64) -> base::Update {
        let message = raw_message(id, chat, None, false);
        base::Update::UpdateNewMessage(Box::new(types::UpdateNewMessage { message, pts: 0, pts_count: 0 }))
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

    /// Routes again what listeners gave back, on `client`'s own router and registry.
    pub(crate) fn redeliver(client: &Client) {
        let router = client.0.handlers.current();
        let mut tasks = Tasks::new(false, None);
        (tasks.on_error, tasks.events) = (router.on_error.clone(), client.0.events.clone());
        router.redeliver(client, &mut tasks);
    }

    /// Whether a listener in `book` still holds what it matches.
    pub(crate) fn holding<T>(book: &Book<T>) -> bool {
        book.shelf().entries.iter().any(|e| matches!(e.sink, Sink::Hold(_)))
    }

    pub(crate) fn entries<T>(book: &Book<T>) -> usize {
        book.waiting.load(Ordering::Acquire)
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

    /// Waits until `book` has a listener and none of them still holds what it matches.
    pub(crate) async fn until_released<T>(book: &Book<T>) {
        for _ in 0..1000 {
            let released = {
                let shelf = book.shelf();
                !shelf.entries.is_empty() && !shelf.entries.iter().any(|e| matches!(e.sink, Sink::Hold(_)))
            };
            if released {
                return;
            }
            tokio::task::yield_now().await;
        }
        panic!("the listener was never released");
    }

    fn ids(returned: Vec<Returned<Message>>) -> Vec<i32> {
        returned.iter().map(|r| r.item.id).collect()
    }

    fn me() -> Me {
        Me { id: 5, ..Me::default() }
    }

    fn claimed<T>(offer: Offer<T>) -> bool {
        matches!(offer, Offer::Claimed)
    }

    #[test]
    fn a_full_hold_gives_back_its_oldest() {
        let (client, book) = (Client::offline(), Book::<Message>::default());
        let _hold = book.hold(1, Criteria::default()).unwrap();
        let last = i32::try_from(MAX_HELD).unwrap() + 2;
        for id in 1..=last {
            assert!(claimed(book.offer(message(&client, id, 7, None, false), &me())), "message {id}");
        }
        let back = book.take_returned();
        assert!(back.iter().all(|r| r.evicted), "pushed out by the cap");
        assert_eq!(ids(back), [1, 2]);
        let shelf = book.shelf();
        let Sink::Hold(held) = &shelf.entries[0].sink else { panic!("the hold ended") };
        assert_eq!((held.len(), held[0].id, held[MAX_HELD - 1].id), (MAX_HELD, 3, last));
    }

    #[tokio::test]
    async fn a_message_pushed_out_of_a_full_hold_skips_every_hold() {
        let client = Client::offline();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        client.on_message(crate::filters::all(), move |m: Message| {
            log.lock().unwrap().push(m.id);
            async { Ok(()) }
        });
        let messages = &client.0.handlers.listeners.messages;
        let chat = || Criteria { chats: vec![7], ..Criteria::default() };
        let (_full, _other) = (messages.hold(1, chat()).unwrap(), messages.hold(2, chat()).unwrap());
        for id in 1..=i32::try_from(MAX_HELD).unwrap() + 1 {
            dispatch(&client, new_message(id, 7));
        }
        redeliver(&client);
        for _ in 0..20 {
            tokio::task::yield_now().await;
        }
        assert_eq!(*seen.lock().unwrap(), [1], "the handlers get it");
        let shelf = messages.shelf();
        let held: Vec<usize> =
            shelf.entries.iter().map(|e| if let Sink::Hold(held) = &e.sink { held.len() } else { 0 }).collect();
        assert_eq!(held, [MAX_HELD, 0], "the other hold doesn't take it");
    }

    #[tokio::test]
    async fn a_hold_keeps_its_matches_until_released() {
        let (client, book) = (Client::offline(), Book::<Message>::default());
        let hold = book.hold(1, Criteria { chats: vec![7], ..Criteria::default() }).unwrap();
        assert!(matches!(book.offer(message(&client, 2, 8, None, false), &me()), Offer::Unclaimed(_)));
        for id in [3, 5] {
            assert!(claimed(book.offer(message(&client, id, 7, None, false), &me())), "message {id}");
        }
        assert!(book.waiting(), "a match doesn't end a hold");
        // Nothing held is newer than message 5: both go back, and the listener waits for the next one.
        let offer = async {
            until_released(&book).await;
            assert!(claimed(book.offer(message(&client, 6, 7, None, false), &me())));
        };
        let account = me();
        let (got, ()) = tokio::join!(book.release(1, (7, 5), None, &account), offer);
        assert_eq!(got.unwrap().id, 6);
        drop(hold);
        assert_eq!(ids(book.take_returned()), [3, 5]);
        assert!(!book.waiting());

        // A dropped or stopped hold gives back what it kept.
        let hold = book.hold(2, Criteria::default()).unwrap();
        assert!(claimed(book.offer(message(&client, 7, 7, None, false), &me())));
        drop(hold);
        assert!(!book.waiting());
        assert_eq!(ids(book.take_returned()), [7]);
        let _hold = book.hold(3, Criteria::default()).unwrap();
        assert!(claimed(book.offer(message(&client, 8, 7, None, false), &me())));
        assert_eq!(book.stop(&Criteria::default()), 1);
        assert!(matches!(book.release(3, (7, 1), None, &account).await, Err(Error::ListenerStopped)));
        assert_eq!(ids(book.take_returned()), [8]);
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
    fn a_panicking_listener_filter_is_a_reported_miss() {
        let (client, book) = (Client::offline(), Book::<Message>::default());
        let events = client.0.events.record();
        let boom = crate::filters::create(|_: &Message| -> bool { panic!("boom") });
        let _panicking = book.wait(1, Criteria { filter: Some(Arc::new(boom)), ..Criteria::default() }).unwrap();
        let mut plain = book.wait(2, Criteria::default()).unwrap();
        assert!(claimed(book.offer(message(&client, 1, 7, None, false), &me())));
        assert_eq!(plain.try_recv().unwrap().id, 1);
        let event = events.try_recv().unwrap();
        let panicked = matches!(&event, crate::enums::ClientEvent::HandlerPanicked {
            kind: UpdateKind::Message, chat_id: Some(7), message: Some(m) } if m == "boom");
        assert!(panicked, "{event:?}");
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
            (Criteria { after: Some((-3, 9)), ..none() }, &by_7, true),
            (Criteria { after: Some((-3, 10)), ..none() }, &by_7, false),
            (Criteria { after: Some((-4, 10)), ..none() }, &by_7, true),
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
        let mut rx = book.wait(1, Criteria::default()).unwrap();
        assert!(claimed(book.offer(message(&client, 1, 7, None, false), &me())));
        assert_eq!(book.expired(1, &mut rx).await.unwrap().id, 1);

        let mut rx = book.wait(2, Criteria::default()).unwrap();
        assert!(matches!(book.expired(2, &mut rx).await, Err(Error::ListenerTimeout)));
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

    fn limited(max: usize) -> Client {
        let client = Client::offline();
        client.0.handlers.listeners.limit(max);
        client
    }

    #[tokio::test]
    async fn the_listener_over_the_limit_is_refused() {
        let client = limited(1);
        let listeners = &client.0.handlers.listeners;
        let first = tokio::spawn({
            let client = client.clone();
            async move { client.listen::<Message>().no_timeout().await }
        });
        until_waiting(&listeners.messages).await;
        let refused = client.listen::<CallbackQuery>().no_timeout().await;
        assert!(matches!(refused, Err(Error::ListenerLimitReached(1))), "{refused:?}");
        assert_eq!(listeners.free_slots(), Some(0));
        first.abort();
        let _ = first.await;
        assert_eq!(listeners.free_slots(), Some(1), "a dropped listener kept its slot");
    }

    #[tokio::test(start_paused = true)]
    async fn a_claimed_or_timed_out_listener_frees_its_slot() {
        let client = limited(1);
        let listeners = &client.0.handlers.listeners;
        let waiter = tokio::spawn({
            let client = client.clone();
            async move { client.listen::<Message>().timeout(Duration::from_secs(5)).await }
        });
        until_waiting(&listeners.messages).await;
        assert_eq!(listeners.free_slots(), Some(0));
        assert!(claimed(listeners.messages.offer(message(&client, 1, 7, None, false), &me())));
        assert_eq!(waiter.await.unwrap().unwrap().id, 1);
        assert_eq!(listeners.free_slots(), Some(1));
        let timed_out = client.listen::<Message>().timeout(Duration::from_secs(5)).await;
        assert!(matches!(timed_out, Err(Error::ListenerTimeout)), "{timed_out:?}");
        assert_eq!(listeners.free_slots(), Some(1));
    }

    #[test]
    fn close_frees_every_slot() {
        let listeners = Listeners::default();
        listeners.limit(2);
        let waiting: Vec<_> = (1..=2)
            .map(|id| {
                let criteria = Criteria { permit: listeners.permit().unwrap(), ..Criteria::default() };
                listeners.messages.wait(id, criteria).unwrap()
            })
            .collect();
        assert!(matches!(listeners.permit(), Err(Error::ListenerLimitReached(2))));
        listeners.close();
        assert_eq!(listeners.free_slots(), Some(2));
        drop(waiting);
    }

    #[tokio::test]
    async fn a_listener_dropped_after_the_claim_gives_the_update_back() {
        let (client, listeners) = (Client::offline(), Listeners::default());
        let mut listen = Box::pin(listeners.messages.listen(1, Criteria::default()));
        tokio::select! {
            biased;
            _ = &mut listen => panic!("nothing was offered"),
            () = std::future::ready(()) => {}
        }
        assert!(claimed(listeners.messages.offer(message(&client, 5, 7, Some(7), false), &me())));
        drop(listen);
        let woken = tokio::time::timeout(Duration::from_secs(5), listeners.returned().notified()).await;
        let back: Vec<i32> = listeners.messages.take_returned().iter().map(|r| r.item.id).collect();
        assert!(woken.is_ok(), "the dispatcher wasn't told");
        assert_eq!(back, [5]);

        // A listener that read its update gives nothing back, and after a stop nothing is kept.
        let reader = listeners.messages.listen(2, Criteria::default());
        let offer = async {
            until_waiting(&listeners.messages).await;
            assert!(claimed(listeners.messages.offer(message(&client, 6, 7, Some(7), false), &me())));
        };
        let (read, ()) = tokio::join!(reader, offer);
        assert_eq!(read.unwrap().id, 6);
        listeners.close();
        assert!(listeners.messages.take_returned().is_empty());
    }
}
