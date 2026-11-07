//! A Telegram [MTProto](https://core.telegram.org/mtproto) client for users and bots, written from scratch: it
//! talks to Telegram's main API directly, as a user account or as a bot (an alternative to the Bot API), with its
//! own transport, auth key exchange and encrypted session, generated types for the whole API layer, and a
//! friendly API of methods, handlers and filters on top.
//!
//! ```no_run
//! use vaegram::prelude::*;
//!
//! #[tokio::main]
//! async fn main() -> Result<()> {
//!     let app = Client::builder("my_bot", 12345, "0123456789abcdef0123456789abcdef")
//!         .bot_token("123456:ABC-DEF")
//!         .start()
//!         .await?;
//!
//!     app.on_message(filters::command("start") & filters::private(), |msg| async move {
//!         msg.answer("Hello from vaegram!").await?;
//!         Ok(())
//!     });
//!
//!     app.run().await
//! }
//! ```
//!
//! The `api_id` and `api_hash` come from <https://my.telegram.org>. Without `.bot_token(..)`, `start` asks on stdin
//! for a phone number, the login code and the two-step verification password. The first run writes
//! `my_bot.session`, and later runs log in from it: see [`ClientBuilder`] for other storage and settings. Anything
//! without a friendly method can be called with [`Client::invoke`] and the TL types in [`raw`]. The [`prelude`]
//! brings in the client, filters, and the types and enums most bots use, with one `use`.
//!
//! # Cargo features
//!
//! None is on by default.
//!
//! - `json`: `ToJson` for every TL type and function, and JSON for raw updates and recordings.
//! - `unstable-raw`: the TL objects behind raw updates (`RawUpdate::raw()`, `raw_users()`, `raw_chats()`),
//!   `Incoming::into_updates()` and `UploadedFile::into_raw()`, and the session plumbing below `Client`
//!   (`connection::Connection`, `session::{Session, UpdatesRx, Incoming, Events}`). Experimental: may change or
//!   go away in any release.
//! - Storage engines: `sqlite` (`SqliteStorage`), `redb` (`RedbStorage`), `redis` (`RedisStorage`), `mongodb`
//!   (`MongoStorage`), `s3` (`S3Storage`, an S3-compatible object store) and `vault` (`VaultStorage`, the login in
//!   HashiCorp Vault).
//! - Observability: `metrics` and `opentelemetry`, request observers in the `observe` module.
//! - `regex`: `filters::regex` and `filters::matches`.
//! - Transports over TLS: `websocket` (`ClientBuilder::websocket`), `dns-config` (`ClientBuilder::dns_fallback`
//!   to Telegram's DNS config) and `web-proxy` (`Proxy::Web`, an MTProxy behind an HTTPS relay).

#![warn(missing_docs)]
#![warn(clippy::return_self_not_must_use)]

// Declares a struct `pub` with the `unstable-raw` feature and `pub(crate)` without it: the session plumbing under
// `Client` is experimental, so only that feature lets callers name it.
macro_rules! unstable_raw_pub {
    ($(#[$meta:meta])* struct $($rest:tt)*) => {
        #[cfg(feature = "unstable-raw")]
        $(#[$meta])*
        pub struct $($rest)*
        #[cfg(not(feature = "unstable-raw"))]
        $(#[$meta])*
        pub(crate) struct $($rest)*
    };
}

/// [`Client`] and its [`ClientBuilder`].
pub mod client;
/// The transport under a session: framing, obfuscation, proxies, and the connection settings in
/// [`Options`](connection::Options).
#[cfg_attr(
    not(feature = "unstable-raw"),
    doc = r#"
The connection itself is experimental and needs the `unstable-raw` feature:

```compile_fail,E0603
fn f(_: vaegram::connection::Connection) {}
```
"#
)]
pub mod connection;
pub(crate) mod crypto;
pub(crate) mod dispatcher;
/// Enumerations of the friendly API, such as chat types, parse modes and client events.
pub mod enums;
/// [`Error`], the error every call can fail with, and Telegram's own [`RpcError`].
pub mod errors;
pub(crate) mod file_id;
/// Filters that choose which updates a handler gets; combine them with `&`, `|` and `!`.
pub mod filters;
#[cfg(test)]
mod fuzz;
/// Handler registration: groups, stopping propagation and removing a handler.
///
/// Updates that come back inside the answer to this client's own request, such as the message `send_message` sent
/// or the edit `edit_message_text` made, reach no handler, raw handler or listener: they only move the update state,
/// and the method returns what they hold. The same account's actions from another session or device arrive as
/// usual.
///
/// A panic in a handler, its filter, a raw handler or a lifecycle hook is caught where it happens and sent to
/// [`Client::on_event`] as [`ClientEvent::HandlerPanicked`](enums::ClientEvent::HandlerPanicked), with the route,
/// the chat and the panic's message; [`Client::on_error`] doesn't get it. Update delivery carries on: a panicking
/// handler stops the later groups for its update as an `Err` does, a panicking filter counts as not matching, the
/// raw handlers and the next updates still run, and with `ordered` the chat moves on to its next update.
pub mod handlers;
/// The friendly API methods, all on [`Client`], and the request builders some of them return.
pub mod methods;
#[cfg(any(feature = "metrics", feature = "opentelemetry"))]
pub mod observe;
pub(crate) mod parser;
/// The items most bots and user clients use, to bring in with `use vaegram::prelude::*;`: [`Client`] and
/// [`ClientBuilder`], [`Result`] and [`Error`], the [`filters`] module and [`Filter`](filters::Filter), the update
/// types ([`Message`](types::Message), [`CallbackQuery`](types::CallbackQuery),
/// [`InlineQuery`](types::InlineQuery)), [`Text`](types::Text) and [`ParseMode`](enums::ParseMode), peers
/// ([`PeerRef`](types::PeerRef), [`User`](types::User), [`Chat`](types::Chat), [`ChatType`](enums::ChatType)),
/// [`InputMedia`](types::InputMedia), [`ReplyParameters`](types::ReplyParameters) and the keyboards. [`Result`]
/// keeps a second parameter for the error, so `Result<T, E>` still works with it in scope. Names you import
/// explicitly win over the glob, so `use std::error::Error;` or `use anyhow::Result;` next to it still means that
/// item.
pub mod prelude;
/// Every type and function of Telegram's API, generated from the TL schema, for [`Client::invoke`].
pub mod raw;
/// The encrypted MTProto session: requests, retries, rate limits, observers and replaying recordings.
#[cfg_attr(
    not(feature = "unstable-raw"),
    doc = r#"
The session itself is experimental and needs the `unstable-raw` feature:

```compile_fail,E0603
fn f(_: vaegram::session::Session) {}
```
"#
)]
pub mod session;
/// Where a client keeps its login, update state and known peers, session strings and recordings.
pub mod storage;
/// The friendly types: messages, users, chats, media, keyboards and the rest.
pub mod types;
/// Bot-API-style chat id conversions, and decoders for the compact thumbnails Telegram inlines.
pub mod utils;

pub use client::{Client, ClientBuilder};
pub use errors::{Error, Result, RpcError};
