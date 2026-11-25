//! Unit tests for bots without Telegram: a [`Client`] whose requests never leave the process.
//!
//! [`Client::recording`] makes a bot client that answers the common bot requests itself and records each request
//! it sends; [`Client::scripted`] makes one whose answers you choose, request by request. Both keep their login
//! and peers in memory and never retry: a scripted `FLOOD_WAIT` comes straight back instead of being slept off.
//! [`TestBuilder`] sets the account and the client's settings first. A test client never connects to Telegram: file
//! transfers to its home DC (2) go through the same answers, and anything that would need another DC fails at once.
//! Make them inside a Tokio runtime, such as the one `#[tokio::test]` starts.
//!
//! ```no_run
//! use vaegram::Client;
//! use vaegram::test::CallKind;
//!
//! # async fn f() -> vaegram::Result<()> {
//! let (client, recorder) = Client::recording();
//! let sent = client.send_message(42, "hi").await?;
//! sent.reply("again").await?;
//! assert_eq!(recorder.sent_texts(42), ["hi", "again"]);
//! assert!(recorder.calls().iter().any(|call| matches!(
//!     &call.kind,
//!     CallKind::SendMessage { reply_to_message_id: Some(id), .. } if *id == sent.id
//! )));
//! # Ok(()) }
//! ```
//!
//! # Default answers
//!
//! A recording client, and a scripted one with [`Script::auto_reply`] on, answer these requests with what
//! Telegram would send back, numbering messages, files and `pts` from 1 up (each channel's `pts` apart):
//!
//! - `messages.sendMessage`: the message, sent, with the next id;
//! - `messages.sendMedia` with a photo or a document (an uploaded one or one sent before): the message with that
//!   media, an uploaded file under a new id; other media (polls, locations, contacts, links) has no default answer;
//! - `messages.editMessage`: the message with the new text;
//! - `messages.deleteMessages`, `channels.deleteMessages` and `messages.readHistory`: done;
//! - `messages.setBotCallbackAnswer`, `messages.setInlineBotResults`, `messages.setTyping` and
//!   `bots.setBotCommands`: `true`;
//! - `upload.saveFilePart` and `upload.saveBigFilePart`: `true`, so uploads work;
//! - `messages.sendReaction`: no updates;
//! - `updates.getState`: the current `pts`;
//! - `users.getUsers` and `channels.getChannels`: each user or supergroup asked for, so sending to any id works,
//!   and the account itself for `get_me`.
//!
//! Any other request fails with [`TestError::NoDefaultAnswer`]; answer it with [`Script::reply_next`] or
//! [`Script::on_call`]. Every failure of the kit's own is a [`TestError`] inside `Error::Other`.
//!
//! # Updates
//!
//! [`Recorder::feed`] hands an update built from [`TestMessage`], [`TestCallbackQuery`] or [`TestUpdate`] to the
//! client's handlers and waits for them; see [`Recorder`] for `dispatch` and `settle`, which a handler waiting in
//! `ask` or `listen` needs, and for `push_stream`, which feeds `run` instead. A test client's
//! [`run`](Client::run) takes what `push_stream` pushes and waits for more, as a real client waits on its
//! connection, until [`Client::stop`] or [`Recorder::close_stream`]: start it on a task of its own.
//!
//! ```no_run
//! use vaegram::Client;
//! use vaegram::filters;
//! use vaegram::test::TestMessage;
//!
//! # async fn f() {
//! let (client, recorder) = Client::recording();
//! client.on_message(filters::command("start"), |msg| async move { msg.reply("Welcome!").await });
//! recorder.feed(TestMessage::private(42, "/start")).await;
//! assert_eq!(recorder.sent_texts(42), ["Welcome!"]);
//! # }
//! ```

mod auto_reply;
mod calls;
mod harness;
mod script;
mod updates;

use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicI32, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, Weak};

use tokio::sync::mpsc;

pub use calls::{CallKind, RecordedCall};
pub use script::Reply;
pub use updates::{TestCallbackQuery, TestChat, TestMessage, TestUpdate, TestUser};

use crate::Client;
use crate::client::{Inner, Me, permits};
use crate::connection::Options;
use crate::dispatcher::Handling;
use crate::enums::ParseMode;
use crate::errors::Error;
use crate::raw::{Deserializable, base};
use crate::session::{AutoSleep, Incoming, NoRetries, Retry, Session, UpdatesRx};
use script::Rules;

/// The account a test client is logged in as when [`TestBuilder`] doesn't say.
const DEFAULT_BOT_ID: i64 = 1000;

/// Settings for a test client: the account it's logged in as and the client settings handlers see. Finish with
/// [`recording`](Self::recording) or [`scripted`](Self::scripted).
///
/// ```no_run
/// use vaegram::enums::ParseMode;
/// use vaegram::test::TestBuilder;
///
/// # async fn f() {
/// let (_client, _recorder) = TestBuilder::new().bot(7, Some("my_bot")).parse_mode(ParseMode::Html).recording();
/// # }
/// ```
#[must_use = "call .recording() or .scripted() to get a client"]
pub struct TestBuilder {
    id: i64,
    bot: bool,
    premium: bool,
    username: Option<String>,
    ordered: bool,
    max_concurrent_handlers: Option<usize>,
    parse_mode: ParseMode,
    retry: Retry,
}

impl fmt::Debug for TestBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TestBuilder").field("id", &self.id).field("bot", &self.bot).finish_non_exhaustive()
    }
}

impl Default for TestBuilder {
    fn default() -> Self {
        TestBuilder::new()
    }
}

impl TestBuilder {
    /// A bot with id 1000 and no username, no retries, and the client defaults of [`Client::builder`] otherwise.
    pub fn new() -> Self {
        TestBuilder {
            id: DEFAULT_BOT_ID,
            bot: true,
            premium: false,
            username: None,
            ordered: false,
            max_concurrent_handlers: None,
            parse_mode: ParseMode::Disabled,
            retry: Retry::new(NoRetries),
        }
    }

    /// Logs the client in as the bot `id`, with `username` (without the `@`).
    pub fn bot(mut self, id: i64, username: Option<&str>) -> Self {
        (self.id, self.bot, self.premium) = (id, true, false);
        self.username = username.map(str::to_owned);
        self
    }

    /// Logs the client in as the user account `id`, with Telegram Premium or without.
    pub fn user(mut self, id: i64, premium: bool) -> Self {
        (self.id, self.bot, self.premium) = (id, false, premium);
        self.username = None;
        self
    }

    /// As [`ClientBuilder::ordered`](crate::ClientBuilder::ordered).
    pub fn ordered(mut self, ordered: bool) -> Self {
        self.ordered = ordered;
        self
    }

    /// As [`ClientBuilder::max_concurrent_handlers`](crate::ClientBuilder::max_concurrent_handlers).
    pub fn max_concurrent_handlers(mut self, n: usize) -> Self {
        self.max_concurrent_handlers = Some(n);
        self
    }

    /// As [`ClientBuilder::parse_mode`](crate::ClientBuilder::parse_mode).
    pub fn parse_mode(mut self, mode: ParseMode) -> Self {
        self.parse_mode = mode;
        self
    }

    /// Sleeps off FLOOD_WAITs as `policy` says, where a test client otherwise hands every error straight back. Run
    /// such a test with `#[tokio::test(start_paused = true)]` so the sleeps take no time.
    pub fn auto_sleep(mut self, policy: AutoSleep) -> Self {
        self.retry = Retry::new(policy);
        self
    }

    /// The client and the [`Recorder`] of the requests it sends; it answers the common bot requests itself. Its
    /// `run` waits for pushed updates until `stop` or [`Recorder::close_stream`].
    ///
    /// # Panics
    ///
    /// Outside a Tokio runtime.
    pub fn recording(self) -> (Client, Recorder) {
        let (client, state) = self.build(true);
        (client, Recorder(state))
    }

    /// The client and the [`Script`] that answers its requests; strict until [`Script::auto_reply`] turns the
    /// default answers on. Its `run` waits for pushed updates until `stop` or [`Script::close_stream`].
    ///
    /// # Panics
    ///
    /// Outside a Tokio runtime.
    pub fn scripted(self) -> (Client, Script) {
        let (client, state) = self.build(false);
        (client, Script(Recorder(state)))
    }

    fn build(self, auto_reply: bool) -> (Client, Arc<State>) {
        let (tx, rx) = mpsc::unbounded_channel();
        let me = Identity { id: self.id, bot: self.bot, premium: self.premium, username: self.username };
        let state = Arc::new(State::new(me, rx, auto_reply));
        let options = Options::default().retry(self.retry);
        let driver = state.clone();
        let session = Session::responding(options.clone(), move |name, body, tag| {
            let call = RecordedCall::new(name, tag, body, driver.me.id);
            driver.calls().push(call.clone());
            let _ = tx.send(call.clone());
            let answer = script::answer(&driver, &call);
            if let Ok(body) = &answer {
                driver.deliver(body);
            }
            answer
        });
        let me = &state.me;
        let username = me.username.as_deref().map(Arc::from);
        let me = Me { id: me.id, is_bot: me.bot, premium: Some(me.premium), username };
        let (ordered, handlers, parse_mode) = (self.ordered, self.max_concurrent_handlers, self.parse_mode);
        let mut stream = None;
        let client = Client::local(session, options, |inner| {
            *inner.me.get_mut().unwrap_or_else(PoisonError::into_inner) = me;
            *inner.parse_mode.get_mut().unwrap_or_else(PoisonError::into_inner) = parse_mode;
            inner.ordered = ordered;
            inner.handler_permits = handlers.map(permits);
            // The stream `run` reads, which `Recorder::push_stream` fills.
            let (tx, rx) = UpdatesRx::channel_with(inner.events.clone());
            *inner.updates.get_mut() = rx;
            stream = Some(tx);
        });
        *state.stream.lock().unwrap_or_else(PoisonError::into_inner) = stream;
        let _ = state.client.set(Arc::downgrade(&client.0));
        (client, state)
    }
}

/// The requests a test client sent, from [`Client::recording`], or [`Script::recorder`] of a [`Client::scripted`]
/// one. Clones share them. (Not the [`storage::recording::Recorder`](crate::storage::recording::Recorder) that
/// writes a real client's traffic to a file.)
#[derive(Clone)]
pub struct Recorder(Arc<State>);

impl fmt::Debug for Recorder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Recorder").finish_non_exhaustive()
    }
}

impl Recorder {
    /// Every request sent since the last [`take_calls`](Self::take_calls), in the order they were sent.
    pub fn calls(&self) -> Vec<RecordedCall> {
        self.0.calls().clone()
    }

    /// Every request sent since the last `take_calls`, in the order they were sent, leaving none.
    pub fn take_calls(&self) -> Vec<RecordedCall> {
        std::mem::take(&mut *self.0.calls())
    }

    /// Waits for the next request the client sends, for a handler running on a task of its own. Each request
    /// comes out once, whatever [`calls`](Self::calls) and [`take_calls`](Self::take_calls) took; the first call
    /// gets the first request ever sent. `None` once the client and its clones are dropped and every request came
    /// out.
    pub async fn recv(&self) -> Option<RecordedCall> {
        self.0.next.lock().await.recv().await
    }

    /// The texts of the messages sent to `chat_id` (a [`CallKind::SendMessage`]) since the last
    /// [`take_calls`](Self::take_calls), in order.
    pub fn sent_texts(&self, chat_id: i64) -> Vec<String> {
        self.0
            .calls()
            .iter()
            .filter_map(|call| match &call.kind {
                CallKind::SendMessage { chat_id: to, text, .. } if *to == chat_id => Some(text.clone()),
                _ => None,
            })
            .collect()
    }
}

/// The answers of a [`Client::scripted`] client, and the [`Recorder`] of what it sent: `calls`, `take_calls`,
/// `recv` and `sent_texts` are here too. Clones share them.
///
/// A request is answered by, in order: the next reply queued for its method with
/// [`reply_next`](Self::reply_next) or [`fail_next`](Self::fail_next); the method's [`on_call`](Self::on_call)
/// responder; the default answer when [`auto_reply`](Self::auto_reply) is on. Without any of them it fails with
/// [`TestError::Unscripted`]. Methods are TL names, such as `"messages.sendMessage"`.
#[derive(Clone)]
pub struct Script(Recorder);

impl fmt::Debug for Script {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Script").finish_non_exhaustive()
    }
}

impl Script {
    /// The [`Recorder`] of the requests this script answered, to hand to code that takes one.
    pub fn recorder(&self) -> &Recorder {
        &self.0
    }

    /// As [`Recorder::calls`].
    pub fn calls(&self) -> Vec<RecordedCall> {
        self.0.calls()
    }

    /// As [`Recorder::take_calls`].
    pub fn take_calls(&self) -> Vec<RecordedCall> {
        self.0.take_calls()
    }

    /// As [`Recorder::recv`].
    pub async fn recv(&self) -> Option<RecordedCall> {
        self.0.recv().await
    }

    /// As [`Recorder::sent_texts`].
    pub fn sent_texts(&self, chat_id: i64) -> Vec<String> {
        self.0.sent_texts(chat_id)
    }

    /// Whether requests the script has no answer for get the default answer of a [`Client::recording`] client
    /// (`true`) or fail (`false`, the default).
    pub fn auto_reply(&self, enabled: bool) -> &Self {
        self.rules().auto_reply = enabled;
        self
    }

    /// Answers the next `method` request not yet answered with `reply`. Replies queued for one method go out in
    /// the order they were queued.
    pub fn reply_next(&self, method: &'static str, reply: Reply) -> &Self {
        self.rules().queue(method, reply);
        self
    }

    /// Fails the next `method` request not yet answered with `error`, such as
    /// `RpcError::new(420, "FLOOD_WAIT", Some(5))`.
    pub fn fail_next(&self, method: &'static str, error: impl Into<Error>) -> &Self {
        self.reply_next(method, Reply::Error(error.into()))
    }

    /// Answers every `method` request that no queued reply answers with what `f` returns for it. A second
    /// responder for the same method replaces the first. `f` runs on the client's request task: it mustn't block.
    pub fn on_call(&self, method: &'static str, f: impl Fn(&RecordedCall) -> Reply + Send + Sync + 'static) -> &Self {
        self.rules().respond(method, Arc::new(f));
        self
    }

    /// Answers the next request, whatever its method, with `reply`: TL bytes or an error. These go out before
    /// any other answer, in the order they were pushed.
    ///
    /// Experimental (`unstable-raw`): may change or go away in any release.
    #[cfg(feature = "unstable-raw")]
    pub fn push_raw(&self, reply: crate::Result<Vec<u8>>) -> &Self {
        self.rules().push_raw(reply);
        self
    }

    fn rules(&self) -> MutexGuard<'_, Rules> {
        (self.0).0.rules()
    }
}

/// Why a test client gave no answer to a request. It reaches the caller inside [`Error::Other`]; read it back with
/// `error.downcast_ref::<TestError>()`.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum TestError {
    /// A [`Script`] had no answer for the request: nothing queued, no responder, and auto-replies off.
    Unscripted {
        /// The request's TL name.
        method: &'static str,
    },
    /// The request has no default answer, or not for what it holds (see [the list](self#default-answers)).
    NoDefaultAnswer {
        /// The request's TL name.
        method: &'static str,
    },
    /// A [`Reply`] that doesn't fit the request, such as [`Reply::SentMessage`] for one that sends no message.
    UnfitReply {
        /// The request's TL name.
        method: &'static str,
    },
    /// An [`on_call`](Script::on_call) responder panicked. The client goes on answering later requests.
    ResponderPanicked {
        /// The request's TL name.
        method: &'static str,
        /// The panic's message.
        message: String,
    },
    /// The request needed a connection to data center `dc_id`, which a test client never opens: a file transfer
    /// there, `invoke_in_dc`, or a `*_MIGRATE` error.
    NoConnection {
        /// The data center.
        dc_id: i32,
    },
    /// [`push_stream`](Recorder::push_stream) found no `run` to take the update: none started, or it couldn't
    /// start (a strict [`Script`] without an answer for `users.getUsers` or `updates.getState`), or it ended, or
    /// the stream was closed.
    NoRun,
}

impl fmt::Display for TestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TestError::Unscripted { method } => write!(
                f,
                "unscripted request {method}: answer it with Script::reply_next, Script::fail_next or \
                 Script::on_call, or turn Script::auto_reply on"
            ),
            TestError::NoDefaultAnswer { method } => write!(
                f,
                "{method} has no default answer in a test client: answer it with Script::reply_next or \
                 Script::on_call"
            ),
            TestError::UnfitReply { method } => write!(f, "the scripted reply doesn't fit {method}"),
            TestError::ResponderPanicked { method, message } => {
                write!(f, "the on_call responder for {method} panicked: {message}")
            }
            TestError::NoConnection { dc_id } => write!(f, "a test client opens no connection, here to DC {dc_id}"),
            TestError::NoRun => write!(
                f,
                "no run took the pushed update: start client.run() on a task of its own first, with getMe and \
                 updates.getState answered"
            ),
        }
    }
}

impl std::error::Error for TestError {}

impl From<TestError> for Error {
    fn from(error: TestError) -> Self {
        Error::other(error)
    }
}

/// The account a test client is logged in as.
pub(crate) struct Identity {
    pub(crate) id: i64,
    pub(crate) bot: bool,
    pub(crate) premium: bool,
    pub(crate) username: Option<String>,
}

/// What a test client's request task, its [`Recorder`] and its [`Script`] share.
pub(crate) struct State {
    pub(crate) me: Identity,
    calls: Mutex<Vec<RecordedCall>>,
    next: tokio::sync::Mutex<mpsc::UnboundedReceiver<RecordedCall>>,
    rules: Mutex<Rules>,
    // The last message id and pts handed out, the pts of each channel apart.
    message_id: AtomicI32,
    pts: AtomicI32,
    channel_pts: Mutex<HashMap<i64, i32>>,
    // The last photo or document id handed out.
    file_id: AtomicI64,
    // The last callback query id handed out.
    query_id: AtomicI64,
    // The client, once built; weak, since its requests hold this state.
    client: OnceLock<Weak<Inner>>,
    // Where pushed updates go to `run`; `None` once the stream is closed.
    stream: Mutex<Option<mpsc::Sender<Incoming>>>,
    // What `feed` and `dispatch` hand updates to, made at the first.
    handling: Mutex<Option<Handling>>,
}

impl State {
    fn new(me: Identity, next: mpsc::UnboundedReceiver<RecordedCall>, auto_reply: bool) -> State {
        State {
            me,
            calls: Mutex::new(Vec::new()),
            next: tokio::sync::Mutex::new(next),
            rules: Mutex::new(Rules::new(auto_reply)),
            message_id: AtomicI32::new(0),
            pts: AtomicI32::new(0),
            channel_pts: Mutex::new(HashMap::new()),
            file_id: AtomicI64::new(0),
            query_id: AtomicI64::new(0),
            client: OnceLock::new(),
            stream: Mutex::new(None),
            handling: Mutex::new(None),
        }
    }

    fn calls(&self) -> MutexGuard<'_, Vec<RecordedCall>> {
        self.calls.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn rules(&self) -> MutexGuard<'_, Rules> {
        self.rules.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// A new message id, one past the last.
    pub(crate) fn next_message_id(&self) -> i32 {
        self.message_id.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
    }

    /// Ids from now on come after `id`, which a script handed out.
    pub(crate) fn took_message_id(&self, id: i32) {
        self.message_id.fetch_max(id, Ordering::Relaxed);
    }

    /// A new photo or document id.
    pub(crate) fn next_file_id(&self) -> i64 {
        self.file_id.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
    }

    /// A new callback query id.
    pub(crate) fn next_query_id(&self) -> i64 {
        self.query_id.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
    }

    /// Moves `pts` on by one and returns it.
    pub(crate) fn next_pts(&self) -> i32 {
        self.pts.fetch_add(1, Ordering::Relaxed).wrapping_add(1)
    }

    /// Moves the `pts` of the channel `channel_id` (its TL id) on by one and returns it.
    pub(crate) fn next_channel_pts(&self, channel_id: i64) -> i32 {
        let mut all = self.channel_pts.lock().unwrap_or_else(PoisonError::into_inner);
        let pts = all.entry(channel_id).or_insert(0);
        *pts = pts.wrapping_add(1);
        *pts
    }

    /// The `pts` of the channel `channel_id` (its TL id), unmoved.
    pub(crate) fn channel_pts(&self, channel_id: i64) -> i32 {
        self.channel_pts.lock().unwrap_or_else(PoisonError::into_inner).get(&channel_id).copied().unwrap_or(0)
    }

    pub(crate) fn pts(&self) -> i32 {
        self.pts.load(Ordering::Relaxed)
    }

    /// Hands an answer that is updates to a `run` going, as a real session hands those of every request: its
    /// tracker learns the `pts` they moved, and no gap shows before the next pushed update.
    fn deliver(&self, body: &[u8]) {
        let Ok(updates) = base::Updates::from_bytes(body) else { return };
        if let Some(stream) = &*self.stream.lock().unwrap_or_else(PoisonError::into_inner) {
            // Full: `run` is behind or not going; a real session drops them too.
            let _ = stream.try_send(Incoming { updates, from_request: true });
        }
    }
}
