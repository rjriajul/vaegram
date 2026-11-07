use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::Duration;

use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::connection::{Options, Proxy, Transport};
use crate::crypto::rsa::PublicKey;
use crate::dispatcher::cache::Caches;
use crate::dispatcher::listeners::UNEXPECTED_CLICK;
use crate::dispatcher::router::run_hook;
use crate::dispatcher::{Recover, Routes};
use crate::enums::ParseMode;
use crate::errors::{Error, Result};
use crate::handlers::UpdateKind;
use crate::methods::{StreamShared, invoke_sleeping_floods};
use crate::raw::{RemoteCall, Serializable, base, functions};
use crate::session::internals::data_center::{self, Dc};
use crate::session::transfer::{Auth, Lease, TransferPool};
use crate::session::{
    AutoSleep, Call, Events, Observer, RateLimiter, RateLimits, Retry, RetryPolicy, Salt, Session, UpdatesRx,
    init_connection,
};
use crate::storage::recording::Recorder;
use crate::storage::{
    FileStorage, MemoryStorage, PeerCache, SessionData, Storage, Store, StoredState, UpdateState, session_string,
};
use crate::types::{Json, LinkPreviewOptions};

const DEFAULT_DC: i32 = 2;
const LISTENER_TIMEOUT: Duration = Duration::from_secs(300);

/// Settings for a [`Client`], from [`Client::builder`]; [`start`](Self::start) or [`connect`](Self::connect) makes
/// the client.
#[must_use = "call .start() or .connect() to get a client"]
pub struct ClientBuilder {
    name: String,
    api_id: i32,
    api_hash: String,
    session_string: Option<String>,
    test_mode: bool,
    ipv6: bool,
    pfs: bool,
    #[cfg(feature = "dns-config")]
    dns_fallback: bool,
    // Stands in for the fetched DNS config.
    #[cfg(all(test, feature = "dns-config"))]
    dns_config: Option<crate::connection::DnsConfig>,
    login: Login,
    parse_mode: ParseMode,
    link_preview_options: Option<LinkPreviewOptions>,
    options: Options,
    ordered: bool,
    catch_up: bool,
    max_concurrent_handlers: Option<usize>,
    record: Option<PathBuf>,
    engine: Option<Arc<dyn Storage>>,
    workdir: Option<PathBuf>,
    client_platform: String,
    listener_timeout: Option<Duration>,
    unallowed_click_alert: Option<Arc<str>>,
    max_listeners: Option<usize>,
    max_concurrent_transmissions: Option<usize>,
    max_message_cache_size: usize,
    max_topic_cache_size: usize,
    fetch: Fetch,
    max_sticker_set_name_cache_size: usize,
    rate_limits: Option<RateLimits>,
    auto_sleep: AutoSleep,
    retry_policy: Option<Retry>,
    auto_no_updates: bool,
}

#[derive(Default)]
pub(crate) struct Login {
    pub(crate) bot_token: Option<String>,
    pub(crate) phone_number: Option<String>,
    pub(crate) phone_code: Option<String>,
    pub(crate) password: Option<String>,
}

impl ClientBuilder {
    /// Logs in as the bot with this token from @BotFather when `start` finds no saved login. Wins over
    /// [`phone_number`](Self::phone_number).
    pub fn bot_token(mut self, token: impl Into<String>) -> Self {
        self.login.bot_token = Some(token.into());
        self
    }

    /// Logs in as the user with this phone number, in international format, when `start` finds no saved login,
    /// instead of asking for it.
    pub fn phone_number(mut self, phone_number: impl Into<String>) -> Self {
        self.login.phone_number = Some(phone_number.into());
        self
    }

    /// The login code `start` tries first instead of asking; a wrong one is asked for again on stdin.
    pub fn phone_code(mut self, phone_code: impl Into<String>) -> Self {
        self.login.phone_code = Some(phone_code.into());
        self
    }

    /// The two-step verification password `start` tries first instead of asking; a wrong one is asked for again
    /// on stdin.
    pub fn password(mut self, password: impl Into<String>) -> Self {
        self.login.password = Some(password.into());
        self
    }

    /// The device model shown in the account's active sessions, `"vaegram"` by default.
    pub fn device_model(mut self, model: impl Into<String>) -> Self {
        self.options.device.model = model.into();
        self
    }

    /// The operating system version shown in the account's active sessions, the OS name by default.
    pub fn system_version(mut self, version: impl Into<String>) -> Self {
        self.options.device.system_version = version.into();
        self
    }

    /// The application version shown in the account's active sessions, this crate's version by default.
    pub fn app_version(mut self, version: impl Into<String>) -> Self {
        self.options.device.app_version = version.into();
        self
    }

    /// The ISO 639-1 code of the client's language, `en` by default.
    pub fn lang_code(mut self, code: impl Into<String>) -> Self {
        self.options.device.lang_code = code.into();
        self
    }

    /// An official language pack name (`android`, `ios`, `tdesktop`, `macos`, …); Telegram refuses others with
    /// `LANG_PACK_INVALID`. Empty by default.
    pub fn lang_pack(mut self, pack: impl Into<String>) -> Self {
        self.options.device.lang_pack = pack.into();
        self
    }

    /// The device's own language, `lang_code` unless set.
    pub fn system_lang_code(mut self, code: impl Into<String>) -> Self {
        self.options.device.system_lang_code = Some(code.into());
        self
    }

    /// The client's offset from UTC in seconds, which Telegram uses to show times in the user's zone.
    pub fn tz_offset(mut self, seconds: i32) -> Self {
        self.options.device.tz_offset = Some(seconds);
        self
    }

    /// The JSON `initConnection.params` carries, the only way to hand Telegram client details the other options
    /// don't cover. Telegram documents just `tz_offset`, which [`tz_offset`](Self::tz_offset) sets. When both are
    /// given, `params` must be an object: its `tz_offset` key is replaced, or added if it has none. Anything else
    /// fails at `connect` and `start` with `Error::InvalidArgument`.
    pub fn init_connection_params(mut self, params: Json) -> Self {
        self.options.device.params = Some(params);
        self
    }

    /// Logs in with `s`, kept in memory; the last of this, [`storage_engine`](Self::storage_engine) and
    /// [`in_memory`](Self::in_memory) wins.
    pub fn session_string(mut self, s: impl Into<String>) -> Self {
        self.session_string = Some(s.into());
        self.engine = None;
        self
    }

    /// The framing of packets on the wire, [`Transport::Intermediate`] by default.
    pub fn transport(mut self, transport: Transport) -> Self {
        self.options.transport = transport;
        self
    }

    /// Disguise MTProto traffic from DPI. MTProxy connections are always obfuscated.
    pub fn obfuscated(mut self) -> Self {
        self.options.obfuscated = true;
        self
    }

    /// Connects through `proxy`, transfers and CDNs included.
    pub fn proxy(mut self, proxy: Proxy) -> Self {
        self.options.proxy = Some(proxy);
        self
    }

    /// Connects over WebSocket on TLS to `*.web.telegram.org:443`, obfuscated, instead of plain TCP: for networks
    /// that only let HTTPS out. Downloads then skip CDNs, which have no WebSocket endpoint. It can't be combined
    /// with an MTProxy or the full transport; `connect` refuses those.
    #[cfg(feature = "websocket")]
    pub fn websocket(mut self) -> Self {
        self.options.websocket = true;
        self
    }

    /// When neither the saved address nor the built-in one of the account's data center answers, fetches the
    /// config Telegram publishes in DNS (over HTTPS, from Google's resolver and then Cloudflare's), checks its
    /// signature and expiry, and tries the plain addresses it lists for that data center. The fetch and those
    /// addresses go out directly, so with a proxy or a connection factory set the fallback stays off rather than
    /// leak a direct connection; in WebSocket mode the addresses don't apply either, since the WebSocket hosts are
    /// fixed. Off by default.
    #[cfg(feature = "dns-config")]
    pub fn dns_fallback(mut self) -> Self {
        self.dns_fallback = true;
        self
    }

    /// Opens every connection's byte stream, transfers and CDNs included, instead of a TCP connect, for example to
    /// bind a local address with `tokio::net::TcpSocket`. See
    /// [`ConnectionFactory`](crate::connection::ConnectionFactory).
    pub fn connection_factory(mut self, factory: impl crate::connection::ConnectionFactory + 'static) -> Self {
        self.options.connection_factory = Some(Arc::new(factory));
        self
    }

    /// Use Telegram's test servers. A stored session keeps the mode it was created with.
    pub fn test_mode(mut self) -> Self {
        self.test_mode = true;
        self
    }

    /// Connects to the data centers' IPv6 addresses instead of IPv4.
    pub fn ipv6(mut self) -> Self {
        self.ipv6 = true;
        self
    }

    /// Perfect forward secrecy: day-long temporary keys, so a leaked session can't decrypt recorded traffic.
    pub fn pfs(mut self) -> Self {
        self.pfs = true;
        self
    }

    /// Runs one chat's handlers one update at a time, in arrival order (by user for inline
    /// queries). Off by default: in a big group, one slow handler then holds up every member.
    pub fn ordered(mut self, ordered: bool) -> Self {
        self.ordered = ordered;
        self
    }

    /// `false` makes every `run` start from now: what came while the client was stopped is skipped, and so are the
    /// updates Telegram still holds for a bot and sends again after a restart, even ones an earlier run handled.
    /// For a bot, what Telegram dated from the moment `run` was called on is still delivered; updates without a
    /// date, such as deletions and poll votes, that it held at the start are skipped with the old ones. Gaps while
    /// it runs, such as after a reconnect, are still filled. `true` by default.
    pub fn catch_up(mut self, catch_up: bool) -> Self {
        self.catch_up = catch_up;
        self
    }

    /// Caps how many handlers run at once, at least 1; updates past the cap wait for a slot. Once 1024 wait, `run`
    /// takes no more updates until a handler ends: they queue up to `updates_capacity`, and what overflows that is
    /// fetched again with getDifference. No cap by default.
    pub fn max_concurrent_handlers(mut self, n: usize) -> Self {
        self.max_concurrent_handlers = Some(n);
        self
    }

    /// Caps how many `listen`, `ask` and next-step listeners wait at once, the way `max_concurrent_handlers` caps
    /// handlers. One over the cap fails at once with `Error::ListenerLimitReached`; a listener gives its slot back
    /// when it gets its update, times out, is stopped or removed. At least 1; no cap by default.
    pub fn max_listeners(mut self, n: usize) -> Self {
        self.max_listeners = Some(n.max(1));
        self
    }

    /// Caps how many downloads and, separately, how many uploads run at once; the rest wait their turn. A
    /// download holds its slot until it ends, and a `.stream()` until it's dropped or finishes. At least 1; no cap
    /// by default, so every transfer runs at once.
    pub fn max_concurrent_transmissions(mut self, n: usize) -> Self {
        self.max_concurrent_transmissions = Some(n.max(1));
        self
    }

    /// Keeps up to `n` received, sent and edited messages, so `reply_to_message()` and
    /// `CallbackQuery::message()` can answer without asking Telegram. Off (0) by default. Deleted messages
    /// (yours included) leave it, and so do messages whose reactions, views or pins change; polls and business
    /// messages never enter it. Up to `n` polls are kept apart, so `on_poll` gets result updates that leave the poll
    /// out. `n` counts messages, not bytes: each entry also keeps the users and chats of the update it came in, which
    /// a big batch shares among all its messages.
    pub fn max_message_cache_size(mut self, n: usize) -> Self {
        self.max_message_cache_size = n;
        self
    }

    /// Keeps up to `n` forum topics that `Message::topic()` fetched; editing a topic drops it. Off (0) by default.
    pub fn max_topic_cache_size(mut self, n: usize) -> Self {
        self.max_topic_cache_size = n;
        self
    }

    /// Fetches the message each new message replies to, and the one a pin service message pinned, before handlers
    /// see it, so `reply_to_message()` and `pinned_message()` answer without a request; the message cache answers
    /// first when it's on. A fetched reply comes without its own reply. A failed fetch leaves the helper asking
    /// Telegram itself. Business messages skip this: their ids belong to the other account. Off by default.
    pub fn fetch_replies(mut self, enabled: bool) -> Self {
        self.fetch.replies = enabled;
        self
    }

    /// Fetches the forum topic of each new message in a forum before handlers see it, so `topic()` answers without
    /// a request. Bots can't fetch topics by id, so a bot skips this. Off by default.
    pub fn fetch_topics(mut self, enabled: bool) -> Self {
        self.fetch.topics = enabled;
        self
    }

    /// Fetches the story a new message replies to, and the full story behind story media that came without it,
    /// before handlers see the message. Bots can't fetch stories, so a bot skips this. Off by default.
    pub fn fetch_stories(mut self, enabled: bool) -> Self {
        self.fetch.stories = enabled;
        self
    }

    /// Looks up the set name of every sticker in new messages before handlers see them, filling
    /// `Sticker::set_name`: one request per set, then the name cache. Off by default.
    pub fn fetch_stickers(mut self, enabled: bool) -> Self {
        self.fetch.stickers = enabled;
        self
    }

    /// How many sticker set names `fetch_stickers` keeps; 250 by default, 0 to look a set up every time.
    pub fn max_sticker_set_name_cache_size(mut self, n: usize) -> Self {
        self.max_sticker_set_name_cache_size = n;
        self
    }

    /// Spaces requests out on the client's side, so a busy bot slows down instead of earning FLOOD_WAITs: each
    /// category of request has a rate and a burst, and every request also counts against a global one. Off by
    /// default. A rate must be above 0 and finite and a burst at least 1, or `connect` and `start` fail with
    /// `Error::InvalidArgument`; [`Client::rate_limiter`] reads and changes them later.
    pub fn rate_limits(mut self, limits: RateLimits) -> Self {
        self.rate_limits = Some(limits);
        self
    }

    /// Asks Telegram not to send this client updates: every request goes out inside `invokeWithoutUpdates`, and so
    /// does the one that opens each connection, the ones that export a login to another DC and the ones that
    /// import it there. For a client that only calls methods. `run` and the listeners need updates, so they fail
    /// with `Error::InvalidArgument`.
    ///
    /// Telegram describes the wrapper as "Invoke a request without subscribing the used connection for updates
    /// (this is enabled by default for file queries)" (<https://core.telegram.org/method/invokeWithoutUpdates>).
    /// It doesn't document what an unwrapped request does to a connection's subscription, so a client with this
    /// on wraps every request that isn't a file part.
    pub fn no_updates(mut self) -> Self {
        self.options.no_updates = true;
        self
    }

    /// Sends the requests that only read (`get…`, `check…`, `read…`, searches, typing indicators and the like)
    /// inside `invokeWithoutUpdates`, so a client that mostly reads stays out of the update flow while its writes
    /// and `updates.*` calls go as usual. Unlike [`no_updates`](Self::no_updates) it leaves `run` and the listeners
    /// working, and it never touches the request that opens a connection, which is what subscribes it to updates.
    /// Off by default; `no_updates` wins when both are on.
    pub fn auto_no_updates(mut self, on: bool) -> Self {
        self.auto_no_updates = on;
        self
    }

    /// How text sent or edited without its own parse mode is read; [`ParseMode::Disabled`], sending text as it is,
    /// by default.
    pub fn parse_mode(mut self, mode: ParseMode) -> Self {
        self.parse_mode = mode;
        self
    }

    /// The link preview options for every message sent or edited without its own `.link_preview_options(..)`.
    /// Its `url` is ignored: one link for every message makes no sense.
    /// [`Client::set_link_preview_options`] changes them later.
    pub fn link_preview_options(mut self, options: LinkPreviewOptions) -> Self {
        self.link_preview_options = Some(options);
        self
    }

    /// 30 seconds by default; `invoke_with_timeout` overrides it per call.
    pub fn request_timeout(mut self, timeout: Duration) -> Self {
        self.options.request_timeout = timeout;
        self
    }

    /// FLOOD_WAITs up to this long are slept off and retried, at most `max_flood_sleeps` times per call; longer
    /// ones go to the caller as `Error::Rpc`. 60 seconds by default; `Duration::ZERO` hands every one to the caller.
    /// [`retry_policy`](Self::retry_policy) replaces this and `max_flood_sleeps`.
    pub fn flood_sleep_threshold(mut self, threshold: Duration) -> Self {
        self.auto_sleep.threshold = threshold;
        self
    }

    /// How many FLOOD_WAITs one call sleeps off, 5 by default; 0 hands every one to the caller. Lost connections
    /// need no retry setting: they are reconnected and the waiting requests sent again on their own.
    pub fn max_flood_sleeps(mut self, n: usize) -> Self {
        self.auto_sleep.tries = n;
        self
    }

    /// Decides what happens to a failed request, in place of the default [`AutoSleep`] that `flood_sleep_threshold`
    /// and `max_flood_sleeps` set up; [`NoRetries`](crate::session::NoRetries) hands every failure to the caller.
    pub fn retry_policy(mut self, policy: impl RetryPolicy) -> Self {
        self.retry_policy = Some(Retry::new(policy));
        self
    }

    /// Update batches waiting for `run`, 1024 by default and at least 1. When it's full, a batch is dropped,
    /// `ClientEvent::UpdatesDropped` fires and a getDifference fetches what it held.
    pub fn updates_capacity(mut self, n: usize) -> Self {
        self.options.updates_capacity = n.max(1);
        self
    }

    /// Sees every request, including the ones vaegram makes on its own (`getDifference`, `initConnection`).
    /// It runs on the connection's task, so it should return quickly.
    pub fn observer(mut self, f: impl Fn(&Call<'_>) + Send + Sync + 'static) -> Self {
        self.options.observer = Some(Observer(Arc::new(f)));
        self
    }

    /// Replaces `path`, which only its owner can read on Unix; [`recording`](crate::storage::recording) says what
    /// goes in, what's left out and how Windows treats the file.
    pub fn record(mut self, path: impl Into<PathBuf>) -> Self {
        self.record = Some(path.into());
        self
    }

    /// Keeps the login, the update state and known peers in `engine` instead of `<name>.session`; the last of this,
    /// [`session_string`](Self::session_string) and [`in_memory`](Self::in_memory) wins. The engine runs on a
    /// thread of the client's own.
    pub fn storage_engine(mut self, engine: impl Storage) -> Self {
        self.engine = Some(Arc::new(engine));
        self.session_string = None;
        self
    }

    /// Keeps everything in memory only: nothing is written, and the next run logs in again unless the login is
    /// exported with `export_session_string`.
    pub fn in_memory(self) -> Self {
        self.storage_engine(MemoryStorage::new())
    }

    /// The directory the default `<name>.session` file goes in; the working directory if unset. Ignored when
    /// another storage is chosen.
    pub fn workdir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.workdir = Some(dir.into());
        self
    }

    fn default_engine(&self) -> FileStorage {
        match &self.workdir {
            Some(dir) => FileStorage::new(dir.join(&self.name)),
            None => FileStorage::new(Path::new(&self.name)),
        }
    }

    /// The platform web apps are told the client runs on (`android`, `ios`, `tdesktop`, …), `"unknown"` by
    /// default; each request's `.platform()` still overrides it.
    pub fn client_platform(mut self, platform: impl Into<String>) -> Self {
        self.client_platform = platform.into();
        self
    }

    /// How long `listen`, `ask` and next-step handlers wait unless they set their own timeout; 5 minutes by
    /// default.
    pub fn listener_timeout(mut self, timeout: Duration) -> Self {
        self.listener_timeout = Some(timeout);
        self
    }

    /// Listeners wait for as long as it takes unless they set their own timeout.
    pub fn no_listener_timeout(mut self) -> Self {
        self.listener_timeout = None;
        self
    }

    /// The alert other users get when they press a button a callback listener is pinned to, unless the listener
    /// sets its own; "You are not expected to click this button." by default.
    pub fn unallowed_click_alert(mut self, text: impl Into<String>) -> Self {
        let text: String = text.into();
        self.unallowed_click_alert = Some(text.into());
        self
    }

    /// Leaves other users' presses on a pinned button to the handlers, unless the listener sets an alert.
    pub fn no_unallowed_click_alert(mut self) -> Self {
        self.unallowed_click_alert = None;
        self
    }

    /// Connects, and when the storage holds no login, logs in. The builder's bot token, phone number, code and
    /// password are used when set; what is missing is asked on stdin, with prompts on stdout: a phone number or bot
    /// token, the confirmation code, the password, and for a new account the name and the terms of service. Use
    /// [`connect`](Self::connect) with `send_code` and `sign_in` to log in without any console I/O.
    pub async fn start(self) -> Result<Client> {
        let (client, login) = self.open().await?;
        if client.0.me.lock().unwrap().id == 0 {
            client.authorize(login).await?;
        }
        Ok(client)
    }

    /// Connects without logging in: when the storage holds no login, the client is connected but signed out, for
    /// `send_code` and `sign_in`. Nothing is asked on stdin.
    pub async fn connect(self) -> Result<Client> {
        Ok(self.open().await?.0)
    }

    async fn open(mut self) -> Result<(Client, Login)> {
        let device = &self.options.device;
        if device.tz_offset.is_some() && device.params.as_ref().is_some_and(|p| !matches!(p, Json::Object(_))) {
            return Err(Error::InvalidArgument("init_connection_params must be an object to carry tz_offset"));
        }
        if let Some(limits) = &self.rate_limits {
            limits.validate()?;
        }
        self.options.retry = self.retry_policy.take().unwrap_or_else(|| Retry::new(self.auto_sleep));
        if let Some(path) = &self.record {
            self.options.record = Some(Recorder::create(path)?);
        }
        let engine: Arc<dyn Storage> = match (self.engine.take(), &self.session_string) {
            (Some(engine), _) => engine,
            (None, Some(_)) => Arc::new(MemoryStorage::new()),
            (None, None) => Arc::new(self.default_engine()),
        };
        let events = Arc::new(Events::default());
        let store = Arc::new(Store::new(engine, events.clone()));
        store.open().await?;
        let stored = match &self.session_string {
            Some(s) => Some(session_string::parse(s).map_err(Error::InvalidSession)?),
            None => store.load_session().await?,
        };
        let (dc_id, auth_key, user_id, is_bot, test_mode, addr) = match &stored {
            Some(s) => (s.dc_id, Some(s.auth_key), s.user_id, s.is_bot, s.test_mode, s.addr),
            None => (DEFAULT_DC, None, 0, false, self.test_mode, None),
        };
        let built_in = data_center::address(dc_id, test_mode, self.ipv6);
        // A session on localhost (a replay, a test server) is offline by design: every DC is that address, over
        // whichever IP family it has.
        let pinned = addr.filter(|a| a.ip().is_loopback());
        let first = pinned.or_else(|| addr.filter(|a| a.is_ipv6() == self.ipv6)).or(built_in);
        let first = first.ok_or(Error::InvalidDc(dc_id))?;
        let dc = |addr| Dc { id: dc_id, addr, test: test_mode };
        let (state, salt) = store.load_state(user_id).await?;
        let warm = store.warm().await?;
        let saved = auth_key.map(|key| (key, salt));
        let try_addr = |addr| connect(dc(addr), saved, self.api_id, &self.options, events.clone(), self.pfs);
        // A saved address comes from help.getConfig and can go stale.
        let result = match (try_addr(first).await, built_in) {
            (Err(_), Some(built_in)) if built_in != first && pinned.is_none() => try_addr(built_in).await,
            (result, _) => result,
        };
        #[cfg(feature = "dns-config")]
        let result = match result {
            Err(error) if self.dns_fallback && pinned.is_none() && self.dials_directly() => {
                let mut result = Err(error);
                for addr in self.dns_addresses(dc_id, test_mode).await {
                    result = try_addr(addr).await;
                    if result.is_ok() {
                        break;
                    }
                }
                result
            }
            result => result,
        };
        let (session, updates, dc_options) = result?;
        let mut inner = Inner::new(session, updates, store);
        inner.peers.get_mut().unwrap_or_else(PoisonError::into_inner).load(warm);
        inner.api_id = self.api_id;
        inner.api_hash = self.api_hash;
        inner.dcs = RwLock::new(data_center::table(&dc_options, self.ipv6, false));
        inner.media_dcs = RwLock::new(data_center::table(&dc_options, self.ipv6, true));
        inner.test_mode = test_mode;
        inner.pinned = pinned;
        inner.ipv6 = self.ipv6;
        inner.options = self.options;
        inner.parse_mode = Mutex::new(self.parse_mode);
        inner.link_preview_options = Mutex::new(self.link_preview_options);
        inner.state = Mutex::new(state);
        inner.me = Mutex::new(Me { id: user_id, is_bot, premium: None, username: None });
        inner.ordered = self.ordered;
        inner.catch_up = self.catch_up;
        inner.handler_permits = self.max_concurrent_handlers.map(|n| Arc::new(Semaphore::new(n.max(1))));
        inner.client_platform = self.client_platform;
        inner.listener_timeout = self.listener_timeout;
        inner.unallowed_click_alert = self.unallowed_click_alert;
        if let Some(n) = self.max_listeners {
            inner.handlers.listeners.limit(n);
        }
        inner.transmissions = self.max_concurrent_transmissions.map(Transmissions::new);
        let set_names = if self.fetch.stickers { self.max_sticker_set_name_cache_size } else { 0 };
        inner.caches = Caches::new(self.max_message_cache_size, self.max_topic_cache_size).with_set_names(set_names);
        inner.fetch = self.fetch;
        inner.limiter = self.rate_limits.as_ref().map(RateLimiter::new);
        inner.auto_no_updates = self.auto_no_updates;
        let client = Client(Arc::new(inner), None);
        let imported = self.session_string.as_deref().is_some_and(session_string::is_import);
        if imported && user_id == 0 {
            match client.get_me_raw().await {
                Ok(me) => client.set_me(&me),
                // The key isn't signed in: `start` logs in as for a fresh session.
                Err(Error::Rpc(e)) if e.code == 401 => {}
                Err(e) => return Err(e),
            }
        }
        client.save().await?;
        Ok((client, self.login))
    }

    /// Whether connections go straight to a data center's address: not through a proxy, a connection factory or
    /// the fixed WebSocket hosts.
    #[cfg(feature = "dns-config")]
    fn dials_directly(&self) -> bool {
        self.options.proxy.is_none() && self.options.connection_factory.is_none() && !self.options.uses_websocket()
    }

    /// The unexpired plain addresses of data center `dc_id` in Telegram's DNS config; none if it can't be had.
    #[cfg(feature = "dns-config")]
    async fn dns_addresses(&self, dc_id: i32, test_mode: bool) -> Vec<SocketAddr> {
        use crate::connection::DnsConfig;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        #[cfg(test)]
        if let Some(config) = &self.dns_config {
            return config.plain_addresses(dc_id, now);
        }
        DnsConfig::fetch(test_mode).await.map(|config| config.plain_addresses(dc_id, now)).unwrap_or_default()
    }
}

/// A connection to Telegram as one account. Clones are cheap and share the connection, handlers and storage, so
/// hand one to each task. Make one with [`Client::builder`].
#[derive(Clone)]
pub struct Client(pub(crate) Arc<Inner>, pub(crate) Option<i64>);

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Client").finish_non_exhaustive()
    }
}

pub(crate) struct Inner {
    session: RwLock<Session>,
    pub(crate) transfers: TransferPool,
    pub(crate) updates: tokio::sync::Mutex<UpdatesRx>,
    pub(crate) api_id: i32,
    pub(crate) api_hash: String,
    dcs: RwLock<Vec<(i32, SocketAddr)>>,
    media_dcs: RwLock<Vec<(i32, SocketAddr)>>,
    cdn_keys: Mutex<Vec<(i32, PublicKey)>>,
    test_mode: bool,
    ipv6: bool,
    pinned: Option<SocketAddr>,
    pub(crate) store: Arc<Store>,
    pub(crate) options: Options,
    pub(crate) parse_mode: Mutex<ParseMode>,
    pub(crate) link_preview_options: Mutex<Option<LinkPreviewOptions>>,
    pub(crate) state: Mutex<Option<UpdateState>>,
    // Bumped by log_out under the `state` lock, so a dispatcher started for the old account can't hand its
    // state back.
    pub(crate) account: AtomicU64,
    pub(crate) me: Mutex<Me>,
    pub(crate) peers: Mutex<PeerCache>,
    pub(crate) handlers: Routes,
    pub(crate) ordered: bool,
    // Whether `run` resumes from the saved update state or starts from now.
    pub(crate) catch_up: bool,
    pub(crate) handler_permits: Option<Arc<Semaphore>>,
    // The default platform of web view requests.
    pub(crate) client_platform: String,
    // What listener builders start from.
    pub(crate) listener_timeout: Option<Duration>,
    pub(crate) unallowed_click_alert: Option<Arc<str>>,
    // Slots for `max_concurrent_transmissions`, if set.
    pub(crate) transmissions: Option<Transmissions>,
    pub(crate) limiter: Option<RateLimiter>,
    pub(crate) auto_no_updates: bool,
    pub(crate) events: Arc<Events>,
    pub(crate) streams: Mutex<HashMap<i64, Arc<StreamShared>>>,
    pub(crate) stop: tokio::sync::watch::Sender<bool>,
    // Set while a `run` (or a client in `compose`) owns the updates, so a second one is refused.
    pub(crate) running: AtomicBool,
    // Set by `disconnect`: the storage is let go of then, or when the running `run` ends.
    pub(crate) disconnected: AtomicBool,
    // Where `recover_gaps` sends its requests while `run` is taking updates.
    pub(crate) recover: Mutex<Option<tokio::sync::mpsc::UnboundedSender<Recover>>>,
    pub(crate) caches: Caches,
    pub(crate) fetch: Fetch,
}

/// What a client fetches for a new message before its handlers run.
#[derive(Clone, Copy, Default, Debug)]
pub(crate) struct Fetch {
    pub(crate) replies: bool,
    pub(crate) topics: bool,
    pub(crate) stories: bool,
    pub(crate) stickers: bool,
}

impl Fetch {
    pub(crate) fn any(self) -> bool {
        self.replies || self.topics || self.stories || self.stickers
    }
}

pub(crate) struct Transmissions {
    pub(crate) downloads: Arc<Semaphore>,
    pub(crate) uploads: Arc<Semaphore>,
}

impl Transmissions {
    pub(crate) fn new(n: usize) -> Transmissions {
        Transmissions { downloads: Arc::new(Semaphore::new(n)), uploads: Arc::new(Semaphore::new(n)) }
    }
}

impl Client {
    /// Waits for a download slot, if downloads are capped.
    pub(crate) async fn download_slot(&self) -> Option<OwnedSemaphorePermit> {
        self.slot(|t| &t.downloads).await
    }

    pub(crate) async fn upload_slot(&self) -> Option<OwnedSemaphorePermit> {
        self.slot(|t| &t.uploads).await
    }

    async fn slot(&self, pick: fn(&Transmissions) -> &Arc<Semaphore>) -> Option<OwnedSemaphorePermit> {
        let slots = pick(self.0.transmissions.as_ref()?).clone();
        // The semaphores are never closed, so this can't fail.
        slots.acquire_owned().await.ok()
    }
}

pub(crate) use me::Me;

// A public type in a private module, so filters' sealed `Inspect` trait can take it without exporting it.
mod me {
    use std::sync::Arc;

    #[derive(Clone, Debug, Default)]
    pub struct Me {
        pub(crate) id: i64,
        pub(crate) is_bot: bool,
        // Looked up again after a reconnect: the subscription can lapse while the client runs.
        pub(crate) premium: Option<bool>,
        pub(crate) username: Option<Arc<str>>,
    }
}

impl Inner {
    fn new(session: Session, updates: UpdatesRx, store: Arc<Store>) -> Inner {
        Inner {
            session: RwLock::new(session),
            events: updates.events.clone(),
            updates: tokio::sync::Mutex::new(updates),
            api_id: 0,
            api_hash: String::new(),
            dcs: RwLock::new(Vec::new()),
            media_dcs: RwLock::new(Vec::new()),
            cdn_keys: Mutex::new(Vec::new()),
            test_mode: false,
            ipv6: false,
            pinned: None,
            peers: Mutex::new(PeerCache::new(store.clone())),
            store,
            options: Options::default(),
            parse_mode: Mutex::new(ParseMode::Disabled),
            link_preview_options: Mutex::new(None),
            transfers: TransferPool::default(),
            state: Mutex::new(None),
            account: AtomicU64::new(0),
            me: Mutex::new(Me::default()),
            handlers: Routes::default(),
            ordered: false,
            catch_up: true,
            handler_permits: None,
            client_platform: "unknown".into(),
            listener_timeout: Some(LISTENER_TIMEOUT),
            unallowed_click_alert: Some(UNEXPECTED_CLICK.clone()),
            transmissions: None,
            limiter: None,
            auto_no_updates: false,
            streams: Mutex::new(HashMap::new()),
            stop: tokio::sync::watch::Sender::new(false),
            running: AtomicBool::new(false),
            disconnected: AtomicBool::new(false),
            recover: Mutex::new(None),
            caches: Caches::default(),
            fetch: Fetch::default(),
        }
    }

    fn stored(&self, update: Option<UpdateState>) -> StoredState {
        let owner = self.me.lock().unwrap_or_else(PoisonError::into_inner).id;
        let salt = self.session.read().unwrap_or_else(PoisonError::into_inner).salt();
        StoredState::new(owner, salt, update)
    }

    /// Returns once the engine has the state, so a stopped client resumes exactly where it left off.
    pub(crate) async fn save_state(&self, state: &UpdateState) {
        self.store.queue_state(self.stored(Some(state.clone())));
        self.store.flush().await;
    }

    /// Written with the next flush of the store thread.
    pub(crate) fn save_state_in_background(&self, state: &UpdateState) {
        self.store.queue_state(self.stored(Some(state.clone())));
    }

    /// Points the client at `engine`, for tests that check what reaches it.
    #[cfg(test)]
    pub(crate) fn set_store(&mut self, engine: Arc<dyn Storage>) {
        self.store = Arc::new(Store::new(engine, self.events.clone()));
        self.peers.get_mut().unwrap().set_store(self.store.clone());
    }

    /// Saves the update state and the salt, then closes the storage for good, waiting for it at most `DROP_WAIT`.
    /// Once is enough: later calls, the one on drop included, do nothing.
    pub(crate) fn finish_store(&self) {
        let update = self.state.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let state = self.stored(update);
        if state.update.is_some() {
            self.store.finish(Some(state));
        } else {
            // No update state yet, but a logged-in session keeps its salt; the saved update stays as it is.
            if state.owner != 0 {
                self.store.queue_salt(state.owner, state.salt);
            }
            self.store.finish(None);
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.finish_store();
    }
}

impl Client {
    /// Begins building a client named `name`, whose session file is `<name>.session` unless another storage is
    /// chosen; nothing connects until `start` or `connect`.
    /// `api_id` and `api_hash` come from <https://my.telegram.org>.
    pub fn builder(name: impl Into<String>, api_id: i32, api_hash: impl Into<String>) -> ClientBuilder {
        ClientBuilder {
            name: name.into(),
            api_id,
            api_hash: api_hash.into(),
            session_string: None,
            test_mode: false,
            ipv6: false,
            pfs: false,
            #[cfg(feature = "dns-config")]
            dns_fallback: false,
            #[cfg(all(test, feature = "dns-config"))]
            dns_config: None,
            login: Login::default(),
            parse_mode: ParseMode::Disabled,
            link_preview_options: None,
            options: Options::default(),
            ordered: false,
            catch_up: true,
            max_concurrent_handlers: None,
            record: None,
            engine: None,
            workdir: None,
            client_platform: "unknown".into(),
            listener_timeout: Some(LISTENER_TIMEOUT),
            unallowed_click_alert: Some(UNEXPECTED_CLICK.clone()),
            max_listeners: None,
            max_concurrent_transmissions: None,
            max_message_cache_size: 0,
            max_topic_cache_size: 0,
            fetch: Fetch::default(),
            max_sticker_set_name_cache_size: 250,
            rate_limits: None,
            auto_sleep: AutoSleep::default(),
            retry_policy: None,
            auto_no_updates: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn offline() -> Client {
        let updates = UpdatesRx::offline();
        let store = Arc::new(Store::new(Arc::new(MemoryStorage::new()), updates.events.clone()));
        let mut inner = Inner::new(Session::offline(), updates, store);
        inner.me.get_mut().unwrap().is_bot = true;
        Client(Arc::new(inner), None)
    }

    #[cfg(test)]
    pub(crate) fn recording() -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        Client::scripted(Vec::new())
    }

    #[cfg(test)]
    pub(crate) fn scripted(replies: Vec<Result<Vec<u8>>>) -> (Client, tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>) {
        let (session, sent) = Session::scripted(replies);
        (Client::with_session(session), sent)
    }

    /// An offline client whose main and transfer session is `session`.
    #[cfg(test)]
    pub(crate) fn with_session(session: Session) -> Client {
        let client = Client::offline();
        client.0.transfers.insert(client.dc(2).unwrap(), &session);
        *client.0.session.write().unwrap() = session;
        client
    }

    #[cfg(test)]
    pub(crate) fn serve_cdn(&self, dc_id: i32, session: &Session) {
        self.0.dcs.write().unwrap().push((dc_id, SocketAddr::from(([127, 0, 0, 1], 443))));
        self.0.cdn_keys.lock().unwrap().push((dc_id, PublicKey::test()));
        self.0.transfers.insert(self.dc(dc_id).unwrap(), session);
    }

    /// Replaces the options, also on the current session.
    #[cfg(test)]
    pub(crate) fn set_options(&mut self, options: Options) {
        let inner = Arc::get_mut(&mut self.0).expect("set_options needs the only handle");
        inner.session.get_mut().unwrap().set_options(options.clone());
        inner.options = options;
    }

    /// Hands the updates that came inside another reply to the dispatcher, the way the ones on the stream arrive.
    pub(crate) fn feed_updates(&self, updates: base::Updates) {
        self.0.events.nested(updates);
    }

    pub(crate) fn session(&self) -> Session {
        self.0.session.read().unwrap().clone()
    }

    pub(crate) fn set_me(&self, user: &base::User) {
        if let base::User::User(u) = user {
            let username = u.username.as_deref().map(Arc::from);
            *self.0.me.lock().unwrap() = Me { id: u.id, is_bot: u.bot, premium: Some(u.premium), username };
        }
    }

    /// A failed lookup isn't cached; transfers that don't need Premium go on with the regular limits.
    pub(crate) async fn is_premium(&self) -> Result<bool> {
        let reconnected = self.0.events.take_reconnected();
        let known = {
            let mut me = self.0.me.lock().unwrap();
            if reconnected {
                me.premium = None;
            }
            if me.is_bot { Some(false) } else { me.premium }
        };
        if let Some(premium) = known {
            return Ok(premium);
        }
        let me = self.get_me_raw().await?;
        self.set_me(&me);
        Ok(matches!(me, base::User::User(u) if u.premium))
    }

    pub(crate) fn dc(&self, dc_id: i32) -> Result<Dc> {
        if let Some(addr) = self.0.pinned {
            return Ok(Dc { id: dc_id, addr, test: self.0.test_mode });
        }
        data_center::find(&self.0.dcs.read().unwrap(), dc_id, self.0.test_mode, self.0.ipv6)
            .ok_or(Error::InvalidDc(dc_id))
    }

    pub(crate) fn media_dc(&self, dc_id: i32) -> Result<Dc> {
        let media = self.0.media_dcs.read().unwrap().iter().find(|(id, _)| *id == dc_id).map(|&(_, addr)| addr);
        match media.filter(|_| self.0.pinned.is_none()) {
            Some(addr) => Ok(Dc { id: dc_id, addr, test: self.0.test_mode }),
            None => self.dc(dc_id),
        }
    }

    pub(crate) async fn switch_dc(&self, dc_id: i32) -> Result<()> {
        let (inner, events, pfs) = (&self.0, self.0.events.clone(), self.session().pfs());
        let (session, updates, dc_options) =
            connect(self.dc(dc_id)?, None, inner.api_id, &inner.options, events, pfs).await?;
        *self.0.dcs.write().unwrap() = data_center::table(&dc_options, self.0.ipv6, false);
        *self.0.media_dcs.write().unwrap() = data_center::table(&dc_options, self.0.ipv6, true);
        *self.0.session.write().unwrap() = session;
        *self.0.updates.lock().await = updates;
        {
            let mut me = self.0.me.lock().unwrap();
            (me.id, me.premium) = (0, None);
        }
        self.0.transfers.clear();
        self.save().await
    }

    pub(crate) fn forget_account(&self) {
        let mut state = self.0.state.lock().unwrap();
        self.0.account.fetch_add(1, Ordering::Relaxed);
        *state = None;
        drop(state);
        *self.0.me.lock().unwrap() = Me::default();
        self.0.peers.lock().unwrap().clear();
        self.0.transfers.clear();
    }

    pub(crate) async fn transfer_sessions(&self, dc: Dc, count: usize) -> Result<Lease> {
        let home = self.session();
        self.0.transfers.lease(dc, count, Auth::Home(&home, self.0.api_id), &self.0.options, &self.0.events).await
    }

    pub(crate) async fn cdn_sessions(&self, dc: Dc, count: usize) -> Result<Lease> {
        let key = self.cdn_key(dc.id).await?;
        self.0.transfers.lease(dc, count, Auth::Cdn(&key), &self.0.options, &self.0.events).await
    }

    pub(crate) fn forget_cdn(&self, dc: Dc) {
        self.0.transfers.forget(dc);
    }

    async fn cdn_key(&self, dc_id: i32) -> Result<PublicKey> {
        let find = |keys: &[(i32, PublicKey)]| keys.iter().find(|(id, _)| *id == dc_id).map(|(_, key)| key.clone());
        if let Some(key) = find(&self.0.cdn_keys.lock().unwrap()) {
            return Ok(key);
        }
        let base::CdnConfig::CdnConfig(config) = self.invoke(&functions::help::GetCdnConfig {}).await?;
        let keys: Vec<_> = config
            .public_keys
            .iter()
            .filter_map(|base::CdnPublicKey::CdnPublicKey(k)| Some((k.dc_id, PublicKey::from_pem(&k.public_key)?)))
            .collect();
        let key = find(&keys);
        *self.0.cdn_keys.lock().unwrap() = keys;
        key.ok_or(Error::InvalidDc(dc_id))
    }

    /// Has the main connection's ups and downs run the connect hooks, each in its own task so user code never
    /// runs on the connection's. The listener holds a `Weak`, so it doesn't keep the client alive.
    pub(crate) fn watch_link(&self) {
        let client = Arc::downgrade(&self.0);
        self.0.events.on_link(Arc::new(move |up| {
            let Some(inner) = client.upgrade() else { return };
            let kind = if up { UpdateKind::Connect } else { UpdateKind::Disconnect };
            let hook = inner.handlers.current().hooks.get(kind);
            if let Some(hook) = hook {
                tokio::spawn(run_hook(Client(inner, None), kind, hook));
            }
        }));
    }

    /// Sends a request about an inline message to `dc_id`, the DC its id names, which is the only one that
    /// serves it.
    pub(crate) async fn invoke_inline<R: RemoteCall>(&self, dc_id: i32, request: R) -> Result<R::Return> {
        self.invoke_in_dc(dc_id, &request).await
    }

    /// `invoke_in_dc` away from home, on a file connection with the login imported.
    pub(crate) async fn invoke_on_file_connection<R: RemoteCall>(
        &self,
        dc: Dc,
        name: &'static str,
        request: &R,
    ) -> Result<R::Return> {
        let lease = self.transfer_sessions(dc, 1).await?;
        // A reopened file connection hasn't sent its layer, and the answer can carry users.
        let request = init_connection(self.0.api_id, &self.0.options, Borrowed(request));
        invoke_sleeping_floods(&lease.sessions[0], name, &request, self.0.options.request_timeout).await
    }

    pub(crate) fn session_data(&self) -> SessionData {
        let session = self.session();
        let me = self.0.me.lock().unwrap();
        SessionData {
            dc_id: session.dc_id(),
            test_mode: self.0.test_mode,
            addr: self.dc(session.dc_id()).ok().map(|dc| dc.addr),
            api_id: self.0.api_id,
            auth_key: *session.auth_key(),
            user_id: me.id,
            is_bot: me.is_bot,
        }
    }

    pub(crate) async fn save(&self) -> Result<()> {
        Ok(self.0.store.save_session(self.session_data()).await?)
    }
}

async fn connect(
    dc: Dc,
    auth_key: Option<([u8; 256], Salt)>,
    api_id: i32,
    options: &Options,
    events: Arc<Events>,
    pfs: bool,
) -> Result<(Session, UpdatesRx, Vec<base::DcOption>)> {
    let rebind = pfs.then(|| match options.no_updates {
        true => init_connection(api_id, options, quiet(functions::help::GetNearestDc {})).to_bytes(),
        false => init_connection(api_id, options, functions::help::GetNearestDc {}).to_bytes(),
    });
    let (session, updates) = Session::connect(dc, auth_key, options, events, rebind).await?;
    // This is what makes the connection subscribe to updates, so a client that wants none says so here too.
    let base::Config::Config(config) = match options.no_updates {
        true => session.invoke(&init_connection(api_id, options, quiet(functions::help::GetConfig {}))).await?,
        false => session.invoke(&init_connection(api_id, options, functions::help::GetConfig {})).await?,
    };
    Ok((session, updates, config.dc_options))
}

pub(crate) fn quiet<X>(query: X) -> functions::InvokeWithoutUpdates<X> {
    functions::InvokeWithoutUpdates { query }
}

// Wraps a request in initConnection without cloning it.
pub(crate) struct Borrowed<'a, R>(pub(crate) &'a R);

impl<R: Serializable> Serializable for Borrowed<'_, R> {
    fn serialize(&self, buf: &mut Vec<u8>) {
        self.0.serialize(buf);
    }
}

impl<R: RemoteCall> RemoteCall for Borrowed<'_, R> {
    const NAME: &'static str = R::NAME;
    type Return = R::Return;
}

#[cfg(test)]
pub(crate) async fn sent<F: IntoFuture>(rx: &mut tokio::sync::mpsc::UnboundedReceiver<Vec<u8>>, call: F) -> u32 {
    let _ = call.await;
    let body = rx.try_recv().expect("no request sent");
    u32::from_le_bytes(body[..4].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;

    use super::{Client, Me};
    use crate::errors::Error;
    use crate::raw::{base, types};
    use crate::session::test_dc::TestDc;
    use crate::session::{AutoSleep, RateCategory, RateLimits};
    use crate::storage::{FileStorage, MemoryStorage, PeerRecord, Recording, SessionData, Storage, UpdateState};
    use crate::types::{Json, PeerRef};

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("vaegram-client-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn builder_defaults_match_todays_behaviour() {
        let builder = Client::builder("defaults", 1, "hash");
        let options = &builder.options;
        assert_eq!(options.updates_capacity, 1024);
        assert_eq!(builder.auto_sleep, AutoSleep::new().threshold(Duration::from_secs(60)).tries(5));
        assert!(builder.catch_up);
        assert_eq!(builder.client_platform, "unknown");
        assert_eq!(builder.listener_timeout, Some(Duration::from_secs(300)));
        assert_eq!(builder.unallowed_click_alert.as_deref(), Some("You are not expected to click this button."));
        let device = &options.device;
        assert_eq!((device.lang_pack.as_str(), device.system_lang_code.as_deref(), device.tz_offset), ("", None, None));
        let set = Client::builder("set", 1, "hash").lang_pack("android").system_lang_code("fr").tz_offset(60);
        let device = &set.options.device;
        assert_eq!((device.lang_pack.as_str(), device.system_lang_code.as_deref()), ("android", Some("fr")));
        assert_eq!(device.tz_offset, Some(60));
    }

    #[test]
    fn a_zero_update_queue_is_stored_as_one() {
        assert_eq!(Client::builder("zero", 1, "hash").updates_capacity(0).options.updates_capacity, 1);
    }

    #[tokio::test]
    async fn non_object_params_with_tz_offset_are_refused() {
        let refused =
            Client::builder("refused", 1, "hash").in_memory().tz_offset(60).init_connection_params(Json::Null);
        assert!(matches!(refused.connect().await, Err(Error::InvalidArgument(_))));
        let array = Client::builder("refused", 1, "hash").in_memory().init_connection_params(Json::Array(Vec::new()));
        let array = array.tz_offset(1).connect().await;
        assert!(matches!(array, Err(Error::InvalidArgument(_))));
    }

    #[test]
    fn zero_listeners_and_transmissions_are_stored_as_one() {
        let builder = Client::builder("zero", 1, "hash").max_listeners(0).max_concurrent_transmissions(0);
        assert_eq!((builder.max_listeners, builder.max_concurrent_transmissions), (Some(1), Some(1)));
    }

    #[tokio::test]
    async fn rate_limits_are_checked_when_connecting() {
        let limit = |rate, burst| RateLimits::default().limit(RateCategory::Query, rate, burst);
        for limits in
            [limit(0.0, 5.0), limit(-1.0, 5.0), limit(f64::NAN, 5.0), limit(f64::INFINITY, 5.0), limit(1.0, 0.5)]
        {
            let refused = Client::builder("limits", 1, "hash").in_memory().rate_limits(limits).connect().await;
            assert!(matches!(refused, Err(Error::InvalidArgument(_))));
        }
    }

    // The account's DC (99) has no built-in address and its saved one refuses, so only the DNS config's address,
    // the test DC's, can open. With `factory`, every dial goes through a connection factory that records and refuses
    // it. Returns the outcome and what the factory saw.
    #[cfg(feature = "dns-config")]
    async fn open_via_dns(expires: i32, factory: bool) -> (Result<Client, Error>, Vec<std::net::SocketAddr>) {
        use std::net::SocketAddr;

        use crate::connection::{AccessPointRule, DnsConfig, IpPort};
        let dc = TestDc::start([5; 256], false).await;
        let SocketAddr::V4(real) = dc.dc(99).addr else { panic!("the test DC isn't on IPv4") };
        let saved = SessionData {
            dc_id: 99,
            test_mode: false,
            addr: Some("0.0.0.0:1".parse().unwrap()),
            api_id: 1,
            auth_key: [5; 256],
            user_id: 0,
            is_bot: false,
        };
        let session = crate::storage::session_string::export(&saved);
        let mut builder = Client::builder("dns", 1, "hash").session_string(session).dns_fallback();
        let dialed = Arc::new(std::sync::Mutex::new(Vec::new()));
        if factory {
            let seen = dialed.clone();
            builder = builder.connection_factory(move |addr: SocketAddr| {
                seen.lock().unwrap().push(addr);
                async { Err::<tokio::net::TcpStream, _>(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)) }
            });
        }
        let rule = AccessPointRule {
            phone_prefix_rules: String::new(),
            dc_id: 99,
            ips: vec![IpPort { addr: real, secret: None }],
        };
        builder.dns_config = Some(DnsConfig { date: 0, expires, rules: vec![rule] });
        let client = builder.connect().await;
        let dialed = dialed.lock().unwrap().clone();
        (client, dialed)
    }

    #[cfg(feature = "dns-config")]
    #[tokio::test]
    async fn fallback_tries_dns_addresses_after_both_fail() {
        let (client, _) = open_via_dns(i32::MAX, false).await;
        // DC 99 has no built-in address and its saved one refuses, so the DNS config's address is what opened.
        assert_eq!(client.unwrap().session().dc_id(), 99);
    }

    #[cfg(feature = "dns-config")]
    #[tokio::test]
    async fn an_expired_dns_config_is_ignored() {
        let (client, _) = open_via_dns(1, false).await;
        assert!(client.is_err());
    }

    #[cfg(feature = "dns-config")]
    #[tokio::test]
    async fn a_connection_factory_turns_the_fallback_off() {
        let (client, dialed) = open_via_dns(i32::MAX, true).await;
        assert!(client.is_err());
        assert_eq!(dialed, ["0.0.0.0:1".parse::<std::net::SocketAddr>().unwrap()]);
    }

    #[tokio::test]
    async fn no_updates_wraps_user_calls_and_init() {
        use crate::raw::functions::Ping;
        use crate::session::test_dc::{INIT_CONNECTION, INVOKE_WITH_LAYER, INVOKE_WITHOUT_UPDATES, PING};
        let dc = TestDc::start([5; 256], false).await;
        let quiet = Client::builder("quiet", 1, "hash").session_string(dc.session_string([5; 256])).no_updates();
        let client = quiet.connect().await.unwrap();
        client.invoke(&Ping { ping_id: 3 }).await.unwrap();
        let seen: Vec<Vec<u32>> = dc.seen().into_iter().map(|s| s.ids).collect();
        // The request that opens the connection: layer, initConnection, the quiet wrapper, then the config call.
        let opening = seen.iter().find(|ids| ids[0] == INVOKE_WITH_LAYER).expect("no initConnection");
        assert_eq!((opening.len(), opening[1], opening[2]), (4, INIT_CONNECTION, INVOKE_WITHOUT_UPDATES));
        assert!(seen.contains(&vec![INVOKE_WITHOUT_UPDATES, PING]), "{seen:?}");

        let plain = Client::builder("plain", 1, "hash").session_string(dc.session_string([5; 256]));
        let plain = plain.connect().await.unwrap();
        plain.invoke(&Ping { ping_id: 4 }).await.unwrap();
        let after: Vec<Vec<u32>> = dc.seen().into_iter().skip(seen.len()).map(|s| s.ids).collect();
        assert!(after.iter().all(|ids| !ids.contains(&INVOKE_WITHOUT_UPDATES)), "{after:?}");
    }

    #[test]
    fn the_link_listener_does_not_keep_the_client_alive() {
        let client = Client::offline();
        client.on_connect(|_| async { Ok(()) });
        let (weak, events) = (std::sync::Arc::downgrade(&client.0), client.0.events.clone());
        drop(client);
        assert!(weak.upgrade().is_none());
        events.link(true);
    }

    #[tokio::test]
    async fn the_default_file_keeps_the_login_the_state_and_known_peers() {
        let dir = temp_dir("files");
        let (mut client, _) = Client::scripted(Vec::new());
        Arc::get_mut(&mut client.0).unwrap().set_store(Arc::new(FileStorage::new(dir.join("bot"))));
        *client.0.me.lock().unwrap() = Me { id: 1001, is_bot: true, ..Me::default() };
        let peer = types::InputPeerUser { user_id: 777, access_hash: 5 };
        client.0.peers.lock().unwrap().insert(777, base::InputPeer::InputPeerUser(Box::new(peer)));
        let update_state = UpdateState { pts: 50, date: 1, ..Default::default() };
        client.save().await.unwrap();
        client.0.save_state(&update_state).await;
        drop(client);

        let file = FileStorage::new(dir.join("bot"));
        let session = file.load_session().await.unwrap().map(|s| (s.user_id, s.is_bot));
        let state = file.load_state().await.unwrap();
        let peers = file.export_peers(None).await.unwrap();
        let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|e| e.unwrap().file_name()).collect();
        names.sort();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(session, Some((1001, true)));
        assert_eq!(state.map(|s| (s.owner, s.update)), Some((1001, Some(update_state))));
        assert_eq!(peers.iter().map(|p| (p.id, p.access_hash)).collect::<Vec<_>>(), [(777, 5)]);
        assert_eq!(names, ["bot.session", "bot.session.lock"], "the login and its lock, no .state");
    }

    #[tokio::test]
    async fn a_session_file_from_an_older_vaegram_is_refused_untouched() {
        let dir = temp_dir("old-files");
        let old = "VAE_BAQAADA5AQABAgMEBQYHCAkKCwwNDg8QERITFBUWFxgZGhscHR4fICEiIyQlJicoKSorLC0uLzAx";
        std::fs::write(dir.join("bot.session"), old).unwrap();
        let client = Client::builder(dir.join("bot").to_str().unwrap(), 1, "hash").connect().await;
        let left = std::fs::read_to_string(dir.join("bot.session")).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let Err(Error::Io(e)) = client else { panic!("an old file opened") };
        assert!(e.to_string().contains("log in again"), "{e}");
        assert_eq!(left, old);
    }

    #[tokio::test]
    async fn a_storage_engine_is_read_on_connect_and_given_the_login() {
        let dc = TestDc::start([5; 256], false).await;
        let login = storage_session(&dc);
        let engine = Arc::new(Recording::default());
        engine.save_session(&login).await.unwrap();
        let client = Client::builder("custom-store", 1, "hash").storage_engine(engine.clone()).connect().await;
        drop(client.unwrap());
        let calls = engine.calls();
        assert_eq!(calls[..6], ["save_session", "open", "load_session", "load_state", "export_peers", "save_session"]);
        assert_eq!(calls.last(), Some(&"close"));
        assert!(!Path::new("custom-store.session").exists());
    }

    fn storage_session(dc: &TestDc) -> SessionData {
        crate::storage::session_string::parse(&dc.session_string([5; 256])).unwrap()
    }

    #[test]
    fn the_last_of_storage_engine_session_string_and_in_memory_wins() {
        let string = Client::builder("a", 1, "hash").in_memory().session_string("x");
        assert!(string.engine.is_none() && string.session_string.is_some());
        let engine = Client::builder("a", 1, "hash").session_string("x").storage_engine(MemoryStorage::new());
        assert!(engine.engine.is_some() && engine.session_string.is_none());
    }

    #[test]
    fn workdir_joins_the_name() {
        let dir = std::env::temp_dir().join("bots");
        let builder = Client::builder("echo", 1, "hash").workdir(&dir);
        assert_eq!(builder.default_engine().path(), dir.join("echo.session"));
        assert_eq!(Client::builder("a/echo", 1, "hash").default_engine().path(), Path::new("a/echo.session"));
    }

    #[tokio::test]
    async fn a_cold_peer_comes_from_the_store_without_a_request() {
        let engine = Arc::new(MemoryStorage::new());
        engine.update_peers(&[PeerRecord::new(777, 5)]).await.unwrap();
        let (mut client, mut sent) = Client::scripted(Vec::new());
        Arc::get_mut(&mut client.0).unwrap().set_store(engine);
        let peer = client.resolve_peer(777).await.unwrap();
        let expected = base::InputPeer::InputPeerUser(Box::new(types::InputPeerUser { user_id: 777, access_hash: 5 }));
        assert_eq!(peer, expected);
        assert!(sent.try_recv().is_err(), "a request went out");
        assert!(client.0.peers.lock().unwrap().contains_key(&777), "and it stays cached");
    }

    #[tokio::test]
    async fn a_stored_username_survives_a_restart() {
        let dir = temp_dir("usernames");
        let (mut client, _) = Client::scripted(Vec::new());
        Arc::get_mut(&mut client.0).unwrap().set_store(Arc::new(FileStorage::new(dir.join("bot"))));
        *client.0.me.lock().unwrap() = Me { id: 1001, is_bot: true, ..Me::default() };
        client.save().await.unwrap();
        client.0.peers.lock().unwrap().remember(777, 5, &["Alice"], None);
        drop(client);

        let (mut client, mut sent) = Client::scripted(Vec::new());
        Arc::get_mut(&mut client.0).unwrap().set_store(Arc::new(FileStorage::new(dir.join("bot"))));
        let found = client.resolve(&PeerRef::Username("@alice".into())).await;
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(found.unwrap().0, 777);
        assert!(sent.try_recv().is_err(), "a request went out");
    }

    #[tokio::test]
    async fn an_in_memory_client_writes_no_files() {
        assert!(Client::builder("memory", 1, "hash").in_memory().engine.is_some());
        let dc = TestDc::start([5; 256], false).await;
        let dir = temp_dir("memory");
        let builder = Client::builder(dir.join("bot").to_str().unwrap(), 1, "hash").in_memory();
        let client = builder.session_string(dc.session_string([5; 256])).connect().await.unwrap();
        client.0.save_state(&UpdateState { pts: 1, date: 1, ..Default::default() }).await;
        client.save().await.unwrap();
        drop(client);
        let files = std::fs::read_dir(&dir).unwrap().count();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(files, 0);
    }

    #[tokio::test]
    async fn builder_listener_settings_reach_the_client() {
        let dc = TestDc::start([5; 256], false).await;
        let builder = Client::builder("listener-settings", 1, "hash").session_string(dc.session_string([5; 256]));
        let builder = builder.listener_timeout(Duration::from_secs(7)).unallowed_click_alert("not yours");
        let client = builder.client_platform("android").connect().await.unwrap();
        assert_eq!(client.0.listener_timeout, Some(Duration::from_secs(7)));
        assert_eq!(client.0.unallowed_click_alert.as_deref(), Some("not yours"));
        assert_eq!(client.0.client_platform, "android");
        let builder = Client::builder("no-listener-settings", 1, "hash").session_string(dc.session_string([5; 256]));
        let client = builder.no_listener_timeout().no_unallowed_click_alert().connect().await.unwrap();
        assert_eq!((client.0.listener_timeout, client.0.unallowed_click_alert.clone()), (None, None));
    }

    #[tokio::test]
    async fn a_session_string_client_writes_no_files() {
        let dc = TestDc::start([5; 256], false).await;
        let dir = temp_dir("string");
        let builder = Client::builder(dir.join("bot").to_str().unwrap(), 1, "hash");
        let client = builder.session_string(dc.session_string([5; 256])).connect().await.unwrap();
        client.0.save_state(&UpdateState { pts: 1, date: 1, ..Default::default() }).await;
        drop(client);
        let files = std::fs::read_dir(&dir).unwrap().count();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(files, 0);
    }

    #[tokio::test]
    async fn a_failing_store_fails_connect_instead_of_panicking() {
        let result = Client::builder("failing-store", 1, "hash").storage_engine(Recording::failing()).connect().await;
        assert!(matches!(result, Err(Error::Io(_))), "{result:?}");
    }

    // A string from another library: dc, IPv4 address, port and key, with no user id.
    fn imported_string(dc: &TestDc) -> String {
        let std::net::SocketAddr::V4(addr) = dc.dc(2).addr else { unreachable!("TestDc listens on IPv4") };
        let b = [&[2][..], &addr.ip().octets(), &addr.port().to_be_bytes(), &[5; 256]].concat();
        format!("1{}", crate::utils::base64url_encode(&b))
    }

    fn user_lookups(dc: &TestDc) -> usize {
        dc.seen().iter().filter(|s| s.ids.last() == Some(&crate::session::test_dc::GET_USERS)).count()
    }

    #[tokio::test]
    async fn an_imported_string_learns_who_it_is() {
        let dc = TestDc::start([5; 256], false).await;
        dc.sign_in(4242);
        let builder = Client::builder("imported-user", 1, "hash").session_string(imported_string(&dc));
        let client = builder.connect().await.unwrap();
        assert_eq!(client.0.me.lock().unwrap().id, 4242);
        let saved = client.0.store.load_session().await.unwrap().map(|s| s.user_id);
        assert_eq!(saved, Some(4242), "the store gets the account");
        assert_eq!(user_lookups(&dc), 1);

        // Our own strings say who they are, even before sign-in: no lookup.
        drop(dc.client([5; 256]).await);
        assert_eq!(user_lookups(&dc), 1);
    }

    #[tokio::test]
    async fn a_revoked_import_stays_logged_out() {
        let dc = TestDc::start([5; 256], false).await;
        let builder = Client::builder("revoked-import", 1, "hash").session_string(imported_string(&dc));
        let client = builder.connect().await.unwrap();
        assert_eq!((client.0.me.lock().unwrap().id, user_lookups(&dc)), (0, 1));
    }
}
