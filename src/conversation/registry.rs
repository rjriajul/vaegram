use std::any::Any;
use std::collections::HashMap;
use std::fmt;
use std::future::Future;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use tokio::task::AbortHandle;

use super::ConversationUpdate;
use super::handle::Conversation;
use super::inbox::private::{Item, Topic};
use super::inbox::{Feeds, Skips};
use crate::Client;
use crate::dialogue::Scope;
use crate::dispatcher::{panicked, report};
use crate::errors::{Error, Result};
use crate::filters::{self, Filter};
use crate::handlers::{BoxFuture, HandlerError};
use crate::types::Message;

type Start<T> = Arc<dyn Fn(Conversation, T) -> BoxFuture + Send + Sync>;

/// A registered conversation's function, for updates of kind `T`; kept type-erased.
struct Starter<T>(Start<T>);

/// How a registered conversation runs, as [`ConversationRef`] sets it.
#[derive(Clone, Default)]
pub(crate) struct Options {
    pub(crate) scope: Scope,
    /// Each wait's timeout; `None` until set, which means the client's `listener_timeout`.
    pub(crate) timeout: Option<Option<Duration>>,
    pub(crate) parallel: bool,
    pub(crate) ignore: Option<Arc<Filter<Message>>>,
}

struct Def {
    start: Box<dyn Any + Send + Sync>,
    options: Options,
}

/// Whose conversation an update belongs to: the parts its scope keys by, `None` for the others.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Key {
    pub(crate) chat: Option<i64>,
    pub(crate) user: Option<i64>,
    /// The forum topic, under the topic scopes only; `Some(None)` outside topics.
    pub(crate) topic: Option<Option<i32>>,
}

/// Where an update points under one scope.
enum Probe {
    /// At this key.
    Exact(Key),
    /// A press under a topic scope: its topic is unknown, so at every topic of this chat and user.
    AnyTopic { chat: Option<i64>, user: Option<i64> },
}

const SCOPES: [Scope; 5] = [Scope::UserInChat, Scope::Chat, Scope::User, Scope::UserInTopic, Scope::ChatTopic];

impl Key {
    /// The key a conversation `update` starts is under; `None` when the update lacks a chat or a user the scope
    /// needs, came through a business connection, or is a press under a topic scope.
    pub(crate) fn of<T: Item>(scope: Scope, update: &T) -> Option<Key> {
        match Key::probe(scope, update)? {
            Probe::Exact(key) => Some(key),
            Probe::AnyTopic { .. } => None,
        }
    }

    fn probe<T: Item>(scope: Scope, update: &T) -> Option<Probe> {
        if update.via_business() {
            return None;
        }
        let (chat, user) = update.parties();
        let (chat, user, by_topic) = match scope {
            Scope::UserInChat => (Some(chat?), Some(user?), false),
            Scope::Chat => (Some(chat?), None, false),
            Scope::User => (None, Some(user?), false),
            Scope::UserInTopic => (Some(chat?), Some(user?), true),
            Scope::ChatTopic => (Some(chat?), None, true),
        };
        Some(match (by_topic, update.conversation_topic()) {
            (false, _) => Probe::Exact(Key { chat, user, topic: None }),
            (true, Topic::Known(topic)) => Probe::Exact(Key { chat, user, topic: Some(topic) }),
            (true, Topic::Unknown) => Probe::AnyTopic { chat, user },
        })
    }
}

struct Active {
    id: u64,
    name: Arc<str>,
    /// The scope it started under, which a later `ConversationRef::scope` doesn't change.
    scope: Scope,
    parallel: bool,
    feeds: Feeds,
    abort: Option<AbortHandle>,
}

/// The conversations a client knows, and the ones running.
#[derive(Default)]
pub(crate) struct Registry {
    defs: Mutex<HashMap<Arc<str>, Def>>,
    active: Mutex<HashMap<Key, Vec<Active>>>,
    next_id: AtomicU64,
    /// What ended conversations handed back unread, so a conversation entered right after doesn't take it.
    drained: Arc<Skips>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Registry {
    /// Registers `f` under `name`, replacing the conversation of that name and its settings.
    pub(crate) fn define<T, F, Fut, R>(&self, name: &str, f: F) -> ConversationRef<'_>
    where
        T: ConversationUpdate,
        F: Fn(Conversation, T) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<R>> + Send + 'static,
    {
        let start: Start<T> = Arc::new(move |conversation, update| {
            let future = f(conversation, update);
            Box::pin(async move { future.await.map(drop) }) as BoxFuture
        });
        let name: Arc<str> = name.into();
        let def = Def { start: Box::new(Starter(start)), options: Options::default() };
        lock(&self.defs).insert(Arc::clone(&name), def);
        ConversationRef { registry: self, name }
    }

    fn configure(&self, name: &str, f: impl FnOnce(&mut Options)) {
        if let Some(def) = lock(&self.defs).get_mut(name) {
            f(&mut def.options);
        }
    }

    /// The set of updates ended conversations handed back unread, shared by every conversation of the client.
    pub(crate) fn drained(&self) -> Arc<Skips> {
        Arc::clone(&self.drained)
    }

    /// Starts the conversation `name` with `update`, on a task of its own.
    pub(crate) fn enter<T: ConversationUpdate>(&self, client: &Client, name: &str, update: T) -> Result<()> {
        client.require_updates()?;
        let (name, start, options) = {
            let defs = lock(&self.defs);
            let Some((name, def)) = defs.get_key_value(name) else {
                return Err(ConversationError::NotRegistered(name.to_owned()).into());
            };
            let Some(Starter(start)) = def.start.downcast_ref::<Starter<T>>() else {
                return Err(ConversationError::WrongUpdate(name.to_string()).into());
            };
            (Arc::clone(name), Arc::clone(start), def.options.clone())
        };
        let key = Key::of(options.scope, &update).ok_or_else(|| ConversationError::NoKey(name.to_string()))?;
        // Listening starts before the key is claimed, so a conversation that can't start leaves nothing behind but
        // listeners its drop removes.
        let conversation = Conversation::open(client, Arc::clone(&name), key, &options)?;
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        {
            let mut active = lock(&self.active);
            let running = active.get(&key).map(Vec::as_slice).unwrap_or_default();
            if let Some(other) = running.iter().find(|a| !(a.parallel && options.parallel)) {
                let busy = ConversationError::Busy { name: name.to_string(), active: other.name.to_string() };
                return Err(busy.into());
            }
            let feeds = conversation.feeds();
            let (scope, parallel) = (options.scope, options.parallel);
            let entry = Active { id, name: Arc::clone(&name), scope, parallel, feeds, abort: None };
            active.entry(key).or_default().push(entry);
        }

        let failed = update.failure(Error::ListenerStopped);
        let (kind, chat_id) = (failed.kind, failed.chat_id);
        let leave = Leave { client: client.clone(), key, id };
        let task = tokio::spawn(async move {
            let result = start(conversation, update).await;
            let client = leave.client.clone();
            // The key is free before `on_error` runs, so the user can start over at once.
            drop(leave);
            if let Err(error) = result {
                if !quiet(&error) {
                    let on_error = client.0.handlers.current().on_error.clone();
                    report(on_error, client.0.events.clone(), HandlerError { error, ..failed }).await;
                }
            }
        });
        let abort = task.abort_handle();
        let exited = {
            let mut active = lock(&self.active);
            match active.get_mut(&key).and_then(|list| list.iter_mut().find(|a| a.id == id)) {
                Some(entry) => {
                    entry.abort = Some(abort.clone());
                    false
                }
                None => true,
            }
        };
        // `exit_conversation` ran before the abort handle was stored, or the conversation already ended.
        if exited {
            abort.abort();
        }
        let events = client.0.events.clone();
        tokio::spawn(async move {
            if let Err(error) = task.await {
                if error.is_panic() {
                    events.emit(panicked(kind, chat_id, error.into_panic()));
                }
            }
        });
        Ok(())
    }

    /// Ends every conversation `name` running for `update`'s chat and user, each under the scope it started with;
    /// how many there were.
    pub(crate) fn exit<T: Item>(&self, client: &Client, name: &str, update: &T) -> usize {
        let mut ended = Vec::new();
        {
            let mut active = lock(&self.active);
            for scope in SCOPES {
                for key in keys(&active, scope, update) {
                    let Some(running) = active.get_mut(&key) else { continue };
                    let (gone, kept): (Vec<Active>, Vec<Active>) =
                        std::mem::take(running).into_iter().partition(|a| &*a.name == name && a.scope == scope);
                    ended.extend(gone);
                    if kept.is_empty() {
                        active.remove(&key);
                    } else {
                        *running = kept;
                    }
                }
            }
        }
        for entry in &ended {
            // At once, so a conversation entered right after gets the next updates, not this one's listeners.
            entry.feeds.remove(client);
            if let Some(abort) = &entry.abort {
                abort.abort();
            }
        }
        ended.len()
    }

    /// Whether a conversation `name` runs for `update`'s chat and user, under the scope it started with.
    pub(crate) fn is_active<T: Item>(&self, name: &str, update: &T) -> bool {
        let active = lock(&self.active);
        SCOPES.into_iter().any(|scope| {
            let named = |running: &Vec<Active>| running.iter().any(|a| &*a.name == name && a.scope == scope);
            keys(&active, scope, update).iter().any(|key| active.get(key).is_some_and(named))
        })
    }

    fn leave(&self, client: &Client, key: Key, id: u64) {
        let entry = {
            let mut active = lock(&self.active);
            let Some(running) = active.get_mut(&key) else { return };
            let Some(at) = running.iter().position(|a| a.id == id) else { return };
            let entry = running.remove(at);
            if running.is_empty() {
                active.remove(&key);
            }
            entry
        };
        entry.feeds.remove(client);
    }
}

/// The keys `update` points at under `scope`: one, or for a press under a topic scope, every topic of its chat and
/// user that has a conversation.
fn keys<T: Item>(active: &HashMap<Key, Vec<Active>>, scope: Scope, update: &T) -> Vec<Key> {
    match Key::probe(scope, update) {
        None => Vec::new(),
        Some(Probe::Exact(key)) => vec![key],
        Some(Probe::AnyTopic { chat, user }) => {
            active.keys().filter(|k| k.chat == chat && k.user == user && k.topic.is_some()).copied().collect()
        }
    }
}

/// Frees a conversation's key and listeners when its task ends, however it ends: returned, failed, panicked or
/// aborted. The conversation itself may have been moved elsewhere and outlive the task.
struct Leave {
    client: Client,
    key: Key,
    id: u64,
}

impl Drop for Leave {
    fn drop(&mut self) {
        self.client.0.conversations.leave(&self.client, self.key, self.id);
    }
}

/// The ends of a conversation that are no failure: a wait that timed out, a client that stopped, and `halt`.
fn quiet(error: &Error) -> bool {
    matches!(error, Error::ListenerTimeout | Error::ListenerStopped)
        || error.downcast_ref::<ConversationError>() == Some(&ConversationError::Halted)
}

/// A conversation just registered by [`Client::conversation`]: set how it runs. Each setting takes effect at once,
/// for the conversations entered from then on. It borrows the client; dropping it keeps the conversation
/// registered.
pub struct ConversationRef<'a> {
    registry: &'a Registry,
    name: Arc<str>,
}

// The setters take effect at once; `self` comes back only for chaining, so dropping it is fine.
#[allow(clippy::return_self_not_must_use)]
impl ConversationRef<'_> {
    /// Keys the conversation by `scope` instead of [`Scope::UserInChat`]: whose updates it sees, and what "one
    /// conversation at a time" counts. Conversations already running keep the scope they started with. Under the
    /// topic scopes a conversation starts from a message only, and its waits for presses must name the pressed
    /// message with [`message_id`](super::Wait::message_id).
    pub fn scope(self, scope: Scope) -> Self {
        self.registry.configure(&self.name, |o| o.scope = scope);
        self
    }

    /// How long each wait waits, unless the wait sets its own; the client's `listener_timeout` (5 minutes unless
    /// set) by default. A wait that times out fails with `Error::ListenerTimeout`, which ends the conversation
    /// quietly when returned with `?`.
    pub fn timeout(self, timeout: Duration) -> Self {
        self.registry.configure(&self.name, |o| o.timeout = Some(Some(timeout)));
        self
    }

    /// Lets each wait wait for as long as it takes, unless the wait sets its own timeout.
    pub fn no_timeout(self) -> Self {
        self.registry.configure(&self.name, |o| o.timeout = Some(None));
        self
    }

    /// Lets the conversation run beside others under the same key, itself included, when they are parallel too.
    /// The oldest gets each update first; one it doesn't want goes on to the next, then to the handlers. A
    /// parallel conversation keeps nothing for later between its waits. Setting `otherwise` on one of its waits or
    /// form fields makes that wait fail with [`ConversationError::OtherwiseInParallel`], since it would take updates
    /// the other conversations wait for.
    pub fn parallel(self) -> Self {
        self.registry.configure(&self.name, |o| o.parallel = true);
        self
    }

    /// Never takes the messages `filter` passes: they go on to the handlers while the conversation runs, such as
    /// `filters::command("cancel")` for a handler that calls [`Client::exit_conversation`]. Call again to ignore
    /// more.
    pub fn ignore(self, filter: Filter<Message>) -> Self {
        self.registry.configure(&self.name, |o| {
            let ignored = match o.ignore.take() {
                Some(earlier) => (*earlier).clone() | filter,
                None => filter,
            };
            o.ignore = Some(Arc::new(ignored));
        });
        self
    }

    /// Never takes a command (text starting with `/`): every command goes on to the handlers while the
    /// conversation runs, so `/cancel` or `/start` is never taken for an answer.
    pub fn ignore_commands(self) -> Self {
        self.ignore(filters::text_starts_with("/"))
    }
}

impl fmt::Debug for ConversationRef<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConversationRef").field("name", &self.name).finish_non_exhaustive()
    }
}

/// Why a conversation didn't start or go on. It reaches the caller inside `Error::Other`; read it back with
/// `error.downcast_ref::<ConversationError>()`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConversationError {
    /// No conversation is registered under this name.
    NotRegistered(String),
    /// The conversation of this name starts with the other update kind: a message instead of a button press, or
    /// the other way round.
    WrongUpdate(String),
    /// The update can't start the conversation of this name: it lacks the chat or the user the scope keys by (a
    /// press on an inline message has no chat), it is a press under a topic scope (a press doesn't say its topic),
    /// or it came through a business connection.
    NoKey(String),
    /// A conversation already runs under the same key, and they aren't both parallel.
    #[non_exhaustive]
    Busy {
        /// The conversation that was to start.
        name: String,
        /// The one already running.
        active: String,
    },
    /// A wait in a parallel conversation had `otherwise`.
    OtherwiseInParallel,
    /// A wait for button presses in a conversation keyed by forum topic named no message with
    /// [`message_id`](super::Wait::message_id); a press doesn't say its topic, so it would never take one.
    UnpinnedPress,
    /// [`Conversation::halt`] ended the conversation; never reported.
    Halted,
}

impl fmt::Display for ConversationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConversationError::NotRegistered(name) => write!(f, "no conversation is registered as {name:?}"),
            ConversationError::WrongUpdate(name) => {
                write!(f, "the conversation {name:?} starts with another kind of update")
            }
            ConversationError::NoKey(name) => {
                write!(f, "the update has no chat or user for the scope of the conversation {name:?}")
            }
            ConversationError::Busy { name, active } => {
                write!(f, "can't start the conversation {name:?}: {active:?} is already running here")
            }
            ConversationError::OtherwiseInParallel => {
                f.write_str("a wait of a parallel conversation can't have otherwise")
            }
            ConversationError::UnpinnedPress => {
                f.write_str("a conversation keyed by topic waits only for presses on a message it names")
            }
            ConversationError::Halted => f.write_str("the conversation was halted"),
        }
    }
}

impl std::error::Error for ConversationError {}

impl From<ConversationError> for Error {
    fn from(error: ConversationError) -> Self {
        Error::other(error)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use crate::Client;
    use crate::conversation::Conversation;
    use crate::dispatcher::listeners::tests::message;
    use crate::types::Message;

    #[tokio::test]
    async fn a_stopping_client_ends_its_conversations_quietly() {
        let (client, _recorder) = Client::recording();
        let reported = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&reported);
        client.on_error(move |_| {
            flag.store(true, Ordering::SeqCst);
            std::future::ready(())
        });
        client.conversation("wait", |mut conv: Conversation, _start: Message| async move {
            conv.wait().await?;
            Ok(())
        });
        let start = message(&client, 1, 7, Some(7), false);
        client.enter_conversation("wait", start.clone()).unwrap();
        assert!(client.is_conversation_active("wait", &start));

        // What the conversation kept is dropped with the stop, as a listener's is.
        client.0.handlers.listeners.close();
        for _ in 0..1000 {
            if !client.is_conversation_active("wait", &start) {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(!client.is_conversation_active("wait", &start));
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        assert!(!reported.load(Ordering::SeqCst), "a stop isn't an error to report");
    }
}
