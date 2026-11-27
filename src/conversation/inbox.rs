//! The listeners that feed a running conversation, and what they let through.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TrySendError;

use super::registry::{Key, Options};
use super::wait::Answer;
use crate::Client;
use crate::dispatcher::listeners::{Criteria, Feeder};
use crate::errors::Result;
use crate::filters::{self, Filter, Inspect, Subject};
use crate::types::{CallbackQuery, Message};

/// How many updates a conversation keeps unread, both kinds together, before the next ones go on to the handlers.
pub(crate) const INBOX: usize = 64;

/// How many updates a set of [`Skips`] remembers.
const SKIPPED: usize = 256;

pub(crate) mod private {
    use std::sync::Arc;

    use crate::errors::Error;
    use crate::filters::Filter;
    use crate::handlers::HandlerError;
    use crate::types::{CallbackQuery, Message};

    use super::super::wait::Answer;

    /// One update, by kind and id: what a conversation remembers of an update it passed on.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Uid {
        /// A message, by chat and message id.
        Message(i64, i32),
        /// A button press, by query id.
        CallbackQuery(i64),
    }

    /// The forum topic an update is in, as far as it says without a request.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Topic {
        /// In this topic, or outside topics for `None`.
        Known(Option<i32>),
        /// A button press: only the pressed message knows its topic.
        Unknown,
    }

    /// The kinds a wait takes, each with its filter; `Some(None)` takes every update of that kind. `pins` are the
    /// messages whose buttons the wait takes presses of, from `message_id`.
    pub struct Wanted {
        pub(crate) messages: Option<Option<Arc<Filter<Message>>>>,
        pub(crate) callbacks: Option<Option<Arc<Filter<CallbackQuery>>>>,
        pub(crate) pins: Vec<i32>,
    }

    /// What a conversation needs of an update kind; sealed, so only [`Message`] and [`CallbackQuery`] have it.
    pub trait Item: Sized {
        /// The chat and the user the update comes from, as the listeners see them.
        fn parties(&self) -> (Option<i64>, Option<i64>);
        /// The forum topic the update is in.
        fn conversation_topic(&self) -> Topic;
        /// Whether it came through a business connection: then it is never a conversation's.
        fn via_business(&self) -> bool;
        /// The update's kind and id.
        fn uid(&self) -> Uid;
        /// The update as an [`Answer`].
        fn into_answer(self) -> Answer;
        /// The update inside `answer`, or `None` when it holds the other kind.
        fn from_answer(answer: Answer) -> Option<Self>;
        /// A report of `error` from a conversation this update started.
        fn failure(&self, error: Error) -> HandlerError;
        /// A wait for this kind only, through `filter`.
        fn wanted(filter: Option<Arc<Filter<Self>>>) -> Wanted;
    }
}

use private::{Item, Uid, Wanted};

/// A bounded set of updates a conversation must not take: the ones it passed on itself, or, shared by every
/// conversation of a client, the ones an ended conversation handed back unread.
#[derive(Default)]
pub(crate) struct Skips(Mutex<VecDeque<Uid>>);

impl Skips {
    pub(crate) fn add(&self, uid: Uid) {
        let mut skipped = lock(&self.0);
        if skipped.len() == SKIPPED {
            skipped.pop_front();
        }
        skipped.push_back(uid);
    }

    fn has(&self, uid: Uid) -> bool {
        lock(&self.0).contains(&uid)
    }
}

/// The listener ids of one conversation, one per update kind.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Feeds {
    messages: u64,
    callbacks: u64,
}

impl Feeds {
    /// Removes both listeners; removing them again does nothing.
    pub(crate) fn remove(self, client: &Client) {
        let listeners = &client.0.handlers.listeners;
        listeners.messages.remove(self.messages);
        listeners.callbacks.remove(self.callbacks);
    }
}

/// What the listeners took for a conversation and it hasn't read yet, both kinds in the order they came.
pub(crate) struct Inbox(pub(crate) mpsc::Receiver<Answer>);

/// What a conversation's listener takes of one update kind.
enum Mode<T> {
    /// Nothing: updates go on to the handlers.
    Off,
    /// Every update.
    All,
    /// Updates the filter passes, every one without a filter. Once one is in the inbox, everything that `rest`
    /// says, so a burst (an album, a double tap) waits for the conversation instead of slipping past while it wakes
    /// up.
    Only(Option<Arc<Filter<T>>>),
}

struct Modes {
    messages: Mode<Message>,
    callbacks: Mode<CallbackQuery>,
    /// The messages whose button presses the current wait takes; under a topic scope, the only presses taken.
    pins: Vec<i32>,
}

/// What a conversation shares with its listeners, which run in the dispatcher.
pub(crate) struct Shared {
    modes: Mutex<Modes>,
    skipped: Skips,
    /// What ended conversations of the client handed back unread: never taken again by any conversation.
    drained: Arc<Skips>,
    ignore: Option<Arc<Filter<Message>>>,
    /// The forum topic the conversation is keyed by, under the topic scopes.
    topic: Option<Option<i32>>,
    /// Whether the listeners keep what comes while the conversation's own code runs, between waits. Parallel
    /// conversations don't, so an older one never holds up the updates a younger one waits for.
    buffer: bool,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Shared {
    pub(crate) fn new(options: &Options, key: &Key, drained: Arc<Skips>) -> Shared {
        let buffer = !options.parallel;
        let modes = Modes { messages: rest(buffer), callbacks: rest(buffer), pins: Vec::new() };
        Shared {
            modes: Mutex::new(modes),
            skipped: Skips::default(),
            drained,
            ignore: options.ignore.clone(),
            topic: key.topic,
            buffer,
        }
    }

    /// Whether the conversation is keyed by forum topic: then it takes only presses on pinned messages.
    pub(crate) fn by_topic(&self) -> bool {
        self.topic.is_some()
    }

    /// Lets the listeners take what `wanted` asks for until the guard drops; `consume` takes every update of the
    /// wanted kinds, for a wait that answers the ones it doesn't want.
    pub(crate) fn arm(&self, wanted: &Wanted, consume: bool) -> Armed<'_> {
        let mut modes = lock(&self.modes);
        modes.messages = mode(&wanted.messages, consume);
        modes.callbacks = mode(&wanted.callbacks, consume);
        modes.pins.clone_from(&wanted.pins);
        Armed(self)
    }

    fn rest_all(&self, modes: &mut Modes) {
        modes.messages = rest(self.buffer);
        modes.callbacks = rest(self.buffer);
        modes.pins.clear();
    }

    /// An update of this kind just went into the inbox: a wait that wanted only some updates now buffers the rest.
    fn claimed(&self, message: bool) {
        let mut modes = lock(&self.modes);
        let waiting =
            if message { matches!(modes.messages, Mode::Only(_)) } else { matches!(modes.callbacks, Mode::Only(_)) };
        if waiting {
            self.rest_all(&mut modes);
        }
    }

    /// Remembers that the conversation passed `uid` on, so its listeners don't take it again.
    pub(crate) fn skip(&self, uid: Uid) {
        self.skipped.add(uid);
    }

    /// Marks `uid`, which the conversation hands back unread as it ends, so no conversation of the client takes it.
    pub(crate) fn drain(&self, uid: Uid) {
        self.drained.add(uid);
    }

    fn refused(&self, uid: Uid) -> bool {
        self.skipped.has(uid) || self.drained.has(uid)
    }

    fn admits_message(&self, message: &Message) -> bool {
        if self.refused(message.uid()) {
            return false;
        }
        if self.topic.is_some_and(|topic| message.conversation_topic() != private::Topic::Known(topic)) {
            return false;
        }
        if self.ignore.as_ref().is_some_and(|ignore| ignore.check(message)) {
            return false;
        }
        self.admits(message, |modes| &mut modes.messages)
    }

    fn admits_callback(&self, query: &CallbackQuery) -> bool {
        if query.via_business() || self.refused(query.uid()) {
            return false;
        }
        // A wait pinned to messages takes only presses on them. A press doesn't say its topic, so under a topic scope
        // only a pinned wait takes presses at all.
        let pinned = {
            let modes = lock(&self.modes);
            let on_pin = query.message_id().is_some_and(|id| modes.pins.contains(&id));
            on_pin || (modes.pins.is_empty() && !self.by_topic())
        };
        if !pinned {
            return false;
        }
        self.admits(query, |modes| &mut modes.callbacks)
    }

    fn admits<T: Subject + Inspect>(&self, item: &T, pick: fn(&mut Modes) -> &mut Mode<T>) -> bool {
        let filter = match pick(&mut lock(&self.modes)) {
            Mode::Off => return false,
            Mode::All => return true,
            Mode::Only(filter) => filter.clone(),
        };
        // The user's filter runs unlocked: a panic in it must not poison the modes.
        filter.is_none_or(|filter| filter.check(item))
    }
}

fn rest<T>(buffer: bool) -> Mode<T> {
    if buffer { Mode::All } else { Mode::Off }
}

fn mode<T>(wanted: &Option<Option<Arc<Filter<T>>>>, consume: bool) -> Mode<T> {
    match wanted {
        None => Mode::Off,
        Some(_) if consume => Mode::All,
        Some(filter) => Mode::Only(filter.clone()),
    }
}

/// Puts the listeners back to what they take between waits when a wait ends, is dropped or times out.
pub(crate) struct Armed<'a>(&'a Shared);

impl Drop for Armed<'_> {
    fn drop(&mut self) {
        self.0.rest_all(&mut lock(&self.0.modes));
    }
}

/// Moves what a listener of kind `T` takes into the conversation's one inbox, and tells `shared` once it is there.
fn feeder<T: Item + Send + 'static>(tx: mpsc::Sender<Answer>, shared: Arc<Shared>) -> Feeder<T> {
    Box::new(move |item: T| match tx.try_reserve() {
        Ok(slot) => {
            let answer = item.into_answer();
            let message = matches!(answer, Answer::Message(_));
            slot.send(answer);
            shared.claimed(message);
            Ok(())
        }
        Err(TrySendError::Full(())) => Err(TrySendError::Full(item)),
        Err(TrySendError::Closed(())) => Err(TrySendError::Closed(item)),
    })
}

/// Starts the two listeners of a conversation keyed by `key`, one per update kind, feeding one inbox. They share one
/// `max_listeners` slot.
pub(crate) fn open(client: &Client, key: &Key, shared: &Arc<Shared>) -> Result<(Feeds, Inbox)> {
    let listeners = &client.0.handlers.listeners;
    let chats: Vec<i64> = key.chat.into_iter().collect();
    let users: Vec<i64> = key.user.into_iter().collect();
    let (tx, rx) = mpsc::channel(INBOX);
    let feeds = Feeds { messages: listeners.next_id(), callbacks: listeners.next_id() };

    let admit = Arc::clone(shared);
    let message_criteria = Criteria {
        chats: chats.clone(),
        users: users.clone(),
        filter: Some(Arc::new(filters::create(move |m: &Message| admit.admits_message(m)))),
        alert: None,
        permit: listeners.permit()?,
        ..Criteria::default()
    };
    let admit = Arc::clone(shared);
    let callback_criteria = Criteria {
        chats,
        users,
        filter: Some(Arc::new(filters::create(move |q: &CallbackQuery| admit.admits_callback(q)))),
        alert: None,
        ..Criteria::default()
    };
    listeners.messages.feed(feeds.messages, message_criteria, feeder(tx.clone(), Arc::clone(shared)))?;
    if let Err(error) = listeners.callbacks.feed(feeds.callbacks, callback_criteria, feeder(tx, Arc::clone(shared))) {
        listeners.messages.remove(feeds.messages);
        return Err(error);
    }
    Ok((feeds, Inbox(rx)))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::dispatcher::listeners::tests::{message, query};

    fn shared(parallel: bool) -> Arc<Shared> {
        let options = Options { parallel, ..Options::default() };
        let key = Key { chat: Some(7), user: Some(7), topic: None };
        Arc::new(Shared::new(&options, &key, Arc::default()))
    }

    fn only_text() -> Wanted {
        Wanted { messages: Some(Some(Arc::new(filters::text()))), callbacks: None, pins: Vec::new() }
    }

    #[tokio::test]
    async fn a_full_inbox_gives_the_update_back_and_keeps_the_wait_armed() {
        let client = Client::offline();
        let shared = shared(true);
        let (tx, _rx) = mpsc::channel(1);
        tx.try_send(Answer::Message(message(&client, 1, 7, Some(7), false))).unwrap();
        let _armed = shared.arm(&only_text(), false);
        let feed = feeder::<Message>(tx, Arc::clone(&shared));

        let full = feed(message(&client, 2, 7, Some(7), false));
        assert!(matches!(full, Err(TrySendError::Full(m)) if m.id == 2));
        // The update didn't go in, so the wait still takes the next matching one.
        assert!(matches!(lock(&shared.modes).messages, Mode::Only(_)));
    }

    #[tokio::test]
    async fn a_claimed_update_switches_the_wait_to_buffering() {
        let client = Client::offline();
        let shared = shared(false);
        let (tx, mut rx) = mpsc::channel(4);
        let _armed = shared.arm(&only_text(), false);
        let feed = feeder::<Message>(tx.clone(), Arc::clone(&shared));
        assert!(feed(message(&client, 1, 7, Some(7), false)).is_ok());
        assert!(matches!(lock(&shared.modes).messages, Mode::All));
        // Both kinds share one inbox, in the order they came.
        assert!(feeder::<CallbackQuery>(tx, Arc::clone(&shared))(query(&client, 7, 7, 3)).is_ok());
        assert!(matches!(rx.recv().await, Some(Answer::Message(_))));
        assert!(matches!(rx.recv().await, Some(Answer::CallbackQuery(_))));
    }

    #[test]
    fn a_business_press_never_reaches_the_bots_own_conversation() {
        let client = Client::offline();
        let update = crate::raw::types::UpdateBusinessBotCallbackQuery {
            query_id: 1,
            user_id: 7,
            connection_id: "conn".into(),
            message: crate::raw::base::Message::Message(Box::new(Message::bare(5, crate::utils::get_peer(7), 0))),
            reply_to_message: None,
            chat_instance: 3,
            data: Some(b"yes".to_vec()),
        };
        let press = CallbackQuery::parse_business(&client, update, &crate::dispatcher::Entities::default());
        let shared = shared(false);
        // The same user's press in the private chat is the conversation's; through a business connection it isn't.
        assert!(shared.admits_callback(&query(&client, 7, 7, 5)));
        assert!(!shared.admits_callback(&press));
        assert_eq!(Key::of(crate::dialogue::Scope::UserInChat, &press), None);
    }
}
