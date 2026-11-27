//! Webhook mode for a client in Bot API mode: Telegram posts each update to the bot's URL, and the receiver checks
//! the secret token and queues the update for the dispatcher exactly as the `getUpdates` poll does, so handlers run
//! the same either way.

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::Value;

use super::bridge::Transport;
use super::convert::int;
use super::polling::{self, ALLOWED, WEBHOOK_FED};
use super::raw::types as bot;
use crate::Client;
use crate::errors::{Error, Result};
use crate::methods::SetWebhook;

/// The header Telegram sends the secret token in, as header names compare: without case.
const SECRET_HEADER: &str = "x-telegram-bot-api-secret-token";
/// How many update ids the receiver remembers, to drop an update Telegram delivers again.
const RECENT_UPDATES: usize = 4096;
/// The longest a delivery waits for room in a full queue before Telegram is told to retry it later.
const QUEUE_WAIT: Duration = Duration::from_secs(10);
const NOT_BOT_API: &str = "webhooks need a client in Bot API mode (ClientBuilder::bot_api)";

/// The receiving end of the bot's webhook, made by [`Client::webhook`]: it takes the requests Telegram posts to the
/// webhook's URL and hands their updates to the client's handlers, the same ones `run` calls when polling.
///
/// Mount [`handle`](Self::handle) in any HTTP server and drive the handlers with [`run`](Self::run), or let
/// `serve` (feature `webhook-server`) do both on an address. Every request must carry the secret
/// token the receiver was made with, in the `X-Telegram-Bot-Api-Secret-Token` header: set the webhook with the same
/// one through [`register`](Self::register).
///
/// An update is answered with 200 as soon as it is queued for the handlers, not once they ran, so a slow handler
/// never makes Telegram deliver it again. A process that dies loses the updates still queued, as with polling.
///
/// ```no_run
/// # async fn f(client: vaegram::Client) -> vaegram::Result<()> {
/// let secret = std::env::var("WEBHOOK_SECRET").unwrap();
/// let webhook = client.webhook(secret)?;
/// webhook.register("https://bot.example.com/telegram").await?;
/// // Behind a reverse proxy that terminates TLS and forwards to port 8080:
/// # #[cfg(feature = "webhook-server")]
/// webhook.serve("127.0.0.1:8080").await?;
/// # Ok(()) }
/// ```
///
/// Cloning is cheap: every clone is the same receiver.
#[derive(Clone)]
pub struct Webhook(Arc<Receiver>);

struct Receiver {
    client: Client,
    transport: Arc<Transport>,
    secret: String,
    recent: Mutex<Recent>,
}

/// What a webhook request got, from [`Webhook::handle`]; [`status`](Self::status) is the HTTP status to answer
/// Telegram with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WebhookResponse {
    /// 200: the update is queued for the handlers, or dropped as polling drops it, being of a kind vaegram doesn't
    /// turn into handler calls yet.
    Accepted,
    /// 200: this receiver already queued an update with this id; its handlers don't run again.
    Duplicate,
    /// 200: the request had the right secret but no update vaegram can read. Telegram would only send it again, so
    /// it is dropped.
    Unreadable,
    /// 401: the secret token header is missing or wrong. Nothing in the body was read.
    Unauthorized,
    /// 503: no `run` takes updates right now, or the queue stayed full; Telegram delivers the update again later.
    Unavailable,
}

impl WebhookResponse {
    /// The HTTP status code to answer the webhook request with. The body may be empty.
    pub fn status(self) -> u16 {
        match self {
            WebhookResponse::Accepted | WebhookResponse::Duplicate | WebhookResponse::Unreadable => 200,
            WebhookResponse::Unauthorized => 401,
            WebhookResponse::Unavailable => 503,
        }
    }
}

/// Where the bot's webhook stands, from [`Client::get_webhook_info`].
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct WebhookInfo {
    /// The webhook's URL; empty when no webhook is set and the bot takes updates by polling.
    pub url: String,
    /// Whether a certificate was uploaded with the webhook, for a server with a self-signed one.
    pub has_custom_certificate: bool,
    /// How many updates wait to be delivered.
    pub pending_update_count: i32,
    /// The IP address Telegram sends the requests to, if it was fixed instead of resolved through DNS.
    pub ip_address: Option<String>,
    /// When the last delivery failed, as a Unix time.
    pub last_error_date: Option<i32>,
    /// Why the last delivery failed, as Telegram words it.
    pub last_error_message: Option<String>,
    /// When Telegram last failed to sync the updates between its data centers, as a Unix time.
    pub last_synchronization_error_date: Option<i32>,
    /// The most requests Telegram sends the webhook at once.
    pub max_connections: Option<i32>,
    /// The kinds of update the webhook gets; empty for Telegram's default set.
    pub allowed_updates: Vec<String>,
}

impl WebhookInfo {
    pub(crate) fn from_raw(info: bot::WebhookInfo) -> WebhookInfo {
        WebhookInfo {
            url: info.url,
            has_custom_certificate: info.has_custom_certificate,
            pending_update_count: int(info.pending_update_count),
            ip_address: info.ip_address,
            last_error_date: info.last_error_date.map(int),
            last_error_message: info.last_error_message,
            last_synchronization_error_date: info.last_synchronization_error_date.map(int),
            max_connections: info.max_connections.map(int),
            allowed_updates: info.allowed_updates.unwrap_or_default(),
        }
    }
}

/// The update ids taken lately, oldest forgotten first, each `true` once queued and `false` while a delivery of it
/// still waits for room in the queue.
#[derive(Default)]
struct Recent {
    ids: HashMap<i64, bool>,
    order: VecDeque<i64>,
}

/// What the receiver knows of an update id a delivery brings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Seen {
    /// Not seen lately: this delivery takes it.
    New,
    /// Another delivery of it waits for room in the queue, and may still fail.
    InFlight,
    /// Queued already.
    Taken,
}

impl Recent {
    /// What is known of `id`; a new one is remembered as in flight.
    fn begin(&mut self, id: i64) -> Seen {
        match self.ids.get(&id) {
            Some(true) => return Seen::Taken,
            Some(false) => return Seen::InFlight,
            None => {}
        }
        self.ids.insert(id, false);
        self.order.push_back(id);
        if self.order.len() > RECENT_UPDATES {
            let oldest = self.order.pop_front();
            if let Some(oldest) = oldest {
                self.ids.remove(&oldest);
            }
        }
        Seen::New
    }

    /// Marks `id` as queued, so later deliveries of it are duplicates.
    fn queued(&mut self, id: i64) {
        if let Some(queued) = self.ids.get_mut(&id) {
            *queued = true;
        }
    }

    /// Forgets `id`, whose delivery failed, so the next one takes it.
    fn remove(&mut self, id: i64) {
        if self.ids.remove(&id).is_some() {
            self.order.retain(|&taken| taken != id);
        }
    }
}

/// The bridge of a client in Bot API mode, or the error a webhook method returns on any other client.
pub(crate) fn bot_api(client: &Client) -> Result<&Arc<Transport>> {
    client.0.bot_api.as_ref().ok_or(Error::InvalidArgument(NOT_BOT_API))
}

/// A secret token Telegram takes: 1 to 256 characters of `A-Z`, `a-z`, `0-9`, `_` and `-`.
pub(crate) fn check_secret(secret: &str) -> Result<()> {
    let allowed = |b: u8| b.is_ascii_alphanumeric() || b == b'_' || b == b'-';
    if (1..=256).contains(&secret.len()) && secret.bytes().all(allowed) {
        Ok(())
    } else {
        Err(Error::InvalidArgument("a webhook secret token is 1 to 256 characters of A-Z, a-z, 0-9, _ and -"))
    }
}

/// The kinds of update `setWebhook` asks for unless told otherwise: the ones the bridge turns into handler calls.
pub(crate) fn default_allowed_updates() -> Vec<String> {
    ALLOWED.iter().map(|kind| (*kind).to_owned()).collect()
}

/// Compares in time that depends on the lengths alone, not on where the bytes first differ.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |differ, (x, y)| differ | (x ^ y)) == 0
}

impl Webhook {
    pub(crate) fn new(client: &Client, secret: String) -> Result<Webhook> {
        let transport = bot_api(client)?.clone();
        client.require_updates()?;
        check_secret(&secret)?;
        let receiver = Receiver { client: client.clone(), transport, secret, recent: Mutex::new(Recent::default()) };
        Ok(Webhook(Arc::new(receiver)))
    }

    /// [`set_webhook`](Client::set_webhook) with this receiver's secret token already set, so Telegram sends the
    /// one the receiver checks.
    pub fn register(&self, url: impl Into<String>) -> SetWebhook<'_> {
        self.0.client.set_webhook(url).secret_token(self.0.secret.clone())
    }

    /// Takes one request Telegram posted to the webhook: its body and its headers, as `(name, value)` pairs in any
    /// form (a `&http::HeaderMap` works as is). Answer the request with the returned
    /// [`status`](WebhookResponse::status).
    ///
    /// The secret token header is checked first, in constant time, before the body is parsed; with the header more
    /// than once, the first one counts. The body is already read by then, so a server that mounts this should cap
    /// its size; `serve` (feature `webhook-server`) checks the header before reading the body at all. The update
    /// then goes to the queue the handlers take from, so [`run`](Self::run) must be running: until it is, every
    /// update is answered 503 and Telegram sends it again later.
    ///
    /// An update id this receiver queued lately (the last 4096, in this process) is answered 200 without running
    /// the handlers again; one whose first delivery still waits for room in the queue is answered 503, so Telegram
    /// keeps it if that delivery fails. Repeats are caught per receiver: make one and clone it.
    ///
    /// ```no_run
    /// # async fn f(client: vaegram::Client, body: Vec<u8>, secret: Option<String>) -> vaegram::Result<()> {
    /// let webhook = client.webhook("my_secret_token")?;
    /// let headers = secret.iter().map(|value| ("X-Telegram-Bot-Api-Secret-Token", value.as_str()));
    /// let answer = webhook.handle(&body, headers).await;
    /// let status: u16 = answer.status();
    /// # Ok(()) }
    /// ```
    pub async fn handle<K, V>(&self, body: &[u8], headers: impl IntoIterator<Item = (K, V)>) -> WebhookResponse
    where
        K: AsRef<str>,
        V: AsRef<[u8]>,
    {
        if !self.0.authorized(headers) {
            return WebhookResponse::Unauthorized;
        }
        self.0.receive(body).await
    }

    /// Runs the handlers on the updates this receiver takes, as [`Client::run`] does on the ones it polls, until
    /// `stop`; nothing is polled meanwhile, as `getUpdates` fails while a webhook is set. Like `Client::run`, it
    /// fails while the client is already running, and then leaves that run as it was.
    pub async fn run(&self) -> Result<()> {
        // Scoped to this run's own task, so a `Client::run` going on elsewhere keeps polling whatever happens here.
        WEBHOOK_FED.scope((), self.0.client.run()).await
    }

    /// Listens for Telegram's requests on `addr` over plain HTTP and runs the handlers on their updates (feature
    /// `webhook-server`), until `stop`; then it returns what [`run`](Self::run) returned.
    ///
    /// Telegram only posts to HTTPS on ports 443, 80, 88 and 8443, so keep it behind a reverse proxy that terminates
    /// TLS, and register the proxy's public URL; bind it where only the proxy reaches it. It serves at most 256
    /// connections at once, the next ones waiting to be accepted, and takes a body of at most 4 MiB, read only once
    /// the secret token matched. Requests to any path are taken; anything but `POST` gets 405.
    #[cfg(feature = "webhook-server")]
    pub async fn serve(&self, addr: impl tokio::net::ToSocketAddrs) -> Result<()> {
        let listener = tokio::net::TcpListener::bind(addr).await?;
        self.serve_listener(listener).await
    }

    /// [`serve`](Self::serve) on a listener already bound, such as one on port 0 or one handed over by the system.
    #[cfg(feature = "webhook-server")]
    pub async fn serve_listener(&self, listener: tokio::net::TcpListener) -> Result<()> {
        tokio::select! {
            ran = self.run() => ran,
            never = server::accept(listener, self.clone()) => match never {},
        }
    }
}

impl fmt::Debug for Webhook {
    // The secret token stays out.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Webhook").finish_non_exhaustive()
    }
}

impl Receiver {
    /// Whether the first secret token header matches the receiver's.
    fn authorized<K, V>(&self, headers: impl IntoIterator<Item = (K, V)>) -> bool
    where
        K: AsRef<str>,
        V: AsRef<[u8]>,
    {
        headers
            .into_iter()
            .find(|(name, _)| <K as AsRef<str>>::as_ref(name).eq_ignore_ascii_case(SECRET_HEADER))
            .is_some_and(|(_, value)| same(<V as AsRef<[u8]>>::as_ref(&value), self.secret.as_bytes()))
    }

    /// Queues the update in an authorized request's body.
    async fn receive(&self, body: &[u8]) -> WebhookResponse {
        // The secret showed it is Telegram's: what doesn't read now never will, so it isn't sent back to retry.
        let Ok(value) = serde_json::from_slice::<Value>(body) else { return WebhookResponse::Unreadable };
        let Some(id) = value.get("update_id").and_then(Value::as_i64) else { return WebhookResponse::Unreadable };
        if !self.client.0.running.load(Ordering::Acquire) {
            return WebhookResponse::Unavailable;
        }
        let seen = self.recent().begin(id);
        match seen {
            Seen::New => {}
            // Not 200: if the delivery in flight fails, Telegram must still have the update to send again.
            Seen::InFlight => return WebhookResponse::Unavailable,
            Seen::Taken => return WebhookResponse::Duplicate,
        }
        match tokio::time::timeout(QUEUE_WAIT, polling::queue(&self.transport, value)).await {
            Ok(true) => {
                self.recent().queued(id);
                WebhookResponse::Accepted
            }
            // The run ended, or its queue stayed full: forgotten, so Telegram's next delivery of it gets through.
            Ok(false) | Err(_) => {
                self.recent().remove(id);
                WebhookResponse::Unavailable
            }
        }
    }

    fn recent(&self) -> std::sync::MutexGuard<'_, Recent> {
        self.recent.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The built-in HTTP/1.1 server of [`Webhook::serve`].
#[cfg(feature = "webhook-server")]
mod server {
    use std::convert::Infallible;
    use std::sync::Arc;
    use std::time::Duration;

    use http_body_util::{BodyExt, Full, Limited};
    use hyper::body::{Bytes, Incoming};
    use hyper::server::conn::http1;
    use hyper::service::service_fn;
    use hyper::{Method, Request, Response, StatusCode};
    use hyper_util::rt::{TokioIo, TokioTimer};
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::Semaphore;
    use tokio::task::JoinSet;

    use super::Webhook;

    /// The largest body taken; an update is far smaller.
    const MAX_BODY: usize = 4 << 20;
    /// How long a connection may take to send a request's headers.
    const HEADER_TIMEOUT: Duration = Duration::from_secs(30);
    /// The pause after a failed accept, such as one out of file descriptors, before the next.
    const ACCEPT_PAUSE: Duration = Duration::from_millis(100);
    /// The most connections served at once; Telegram opens at most 100 (`max_connections`).
    const MAX_CONNECTIONS: usize = 256;

    /// Serves every connection `listener` accepts, [`MAX_CONNECTIONS`] at a time, until dropped; dropping it ends
    /// the connections too.
    pub(super) async fn accept(listener: TcpListener, webhook: Webhook) -> Infallible {
        let slots = Arc::new(Semaphore::new(MAX_CONNECTIONS));
        let mut connections = JoinSet::new();
        // The semaphore is never closed: each turn waits for a free slot, which its connection gives back as it ends.
        while let Ok(slot) = slots.clone().acquire_owned().await {
            let stream = 'accepted: loop {
                tokio::select! {
                    accepted = listener.accept() => match accepted {
                        Ok((stream, _)) => break 'accepted stream,
                        Err(_) => tokio::time::sleep(ACCEPT_PAUSE).await,
                    },
                    // Reaps the finished ones, so the set doesn't grow.
                    Some(_) = connections.join_next(), if !connections.is_empty() => {}
                }
            };
            let webhook = webhook.clone();
            connections.spawn(async move {
                connection(stream, webhook).await;
                drop(slot);
            });
        }
        std::future::pending().await
    }

    async fn connection(stream: TcpStream, webhook: Webhook) {
        let service = service_fn(move |request: Request<Incoming>| {
            let webhook = webhook.clone();
            async move { Ok::<_, Infallible>(respond(&webhook, request).await) }
        });
        let mut http = http1::Builder::new();
        http.timer(TokioTimer::new()).header_read_timeout(HEADER_TIMEOUT);
        // A connection that breaks or times out just ends; Telegram opens another.
        let _ = http.serve_connection(TokioIo::new(stream), service).await;
    }

    async fn respond(webhook: &Webhook, request: Request<Incoming>) -> Response<Full<Bytes>> {
        if *request.method() != Method::POST {
            return reply(StatusCode::METHOD_NOT_ALLOWED);
        }
        let (parts, body) = request.into_parts();
        // Checked before the body is read, so a stranger can't make the server buffer anything.
        if !webhook.0.authorized(&parts.headers) {
            return reply(StatusCode::UNAUTHORIZED);
        }
        let body = match Limited::new(body, MAX_BODY).collect().await {
            Ok(collected) => collected.to_bytes(),
            Err(_) => return reply(StatusCode::PAYLOAD_TOO_LARGE),
        };
        let answer = webhook.0.receive(&body).await;
        reply(StatusCode::from_u16(answer.status()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR))
    }

    fn reply(status: StatusCode) -> Response<Full<Bytes>> {
        let mut response = Response::new(Full::new(Bytes::new()));
        *response.status_mut() = status;
        response
    }
}

#[cfg(test)]
mod tests {
    use super::{RECENT_UPDATES, Recent, Seen, WebhookResponse, check_secret, same};

    #[test]
    fn secret_tokens_are_checked_as_telegram_does() {
        assert!(check_secret("abc_DEF-123").is_ok());
        assert!(check_secret(&"a".repeat(256)).is_ok());
        for bad in ["", "has space", "dot.", "ümlaut"] {
            assert!(check_secret(bad).is_err(), "{bad:?}");
        }
        assert!(check_secret(&"a".repeat(257)).is_err());
    }

    #[test]
    fn secrets_compare_whole() {
        assert!(same(b"secret", b"secret"));
        assert!(!same(b"secret", b"secreT"));
        assert!(!same(b"secret", b"secret2"));
        assert!(!same(b"", b"x"));
    }

    #[test]
    fn recent_ids_tell_in_flight_from_queued_and_forget_the_oldest() {
        let mut recent = Recent::default();
        assert_eq!(recent.begin(1), Seen::New);
        // Still waiting for room: a second delivery must not be told it was taken.
        assert_eq!(recent.begin(1), Seen::InFlight);
        recent.remove(1);
        assert_eq!(recent.begin(1), Seen::New);
        recent.queued(1);
        assert_eq!(recent.begin(1), Seen::Taken);
        for id in 2..=RECENT_UPDATES as i64 + 1 {
            assert_eq!(recent.begin(id), Seen::New);
        }
        // Id 1 was the oldest of RECENT_UPDATES + 1, so it is forgotten and taken again.
        assert_eq!(recent.begin(1), Seen::New);
        assert_eq!(recent.ids.len(), recent.order.len());
        assert!(recent.ids.len() <= RECENT_UPDATES);
    }

    #[test]
    fn only_a_bad_secret_or_no_room_is_retried() {
        let retried = |r: WebhookResponse| r.status() != 200;
        assert!(!retried(WebhookResponse::Accepted));
        assert!(!retried(WebhookResponse::Duplicate));
        assert!(!retried(WebhookResponse::Unreadable));
        assert_eq!(WebhookResponse::Unauthorized.status(), 401);
        assert_eq!(WebhookResponse::Unavailable.status(), 503);
    }
}
