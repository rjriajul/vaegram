//! A bot that talks to Telegram through the Bot API over HTTPS (feature `bot-api`), instead of an MTProto session.
//!
//! [`BotApi::invoke`] sends any method of [`raw::methods`]; the answers are the types of [`raw::types`], generated
//! from the Bot API's specification by `tools/botapi/gen.py`.
//!
//! [`ClientBuilder::bot_api`](crate::ClientBuilder::bot_api) goes further: a whole [`Client`](crate::Client) over
//! the Bot API, with the same friendly methods, types and handlers as over MTProto. A request it can't map yet
//! fails with [`Unsupported`].
//!
//! Experimental: part of the 0.4.0 nightlies and may change in any of them.

pub(crate) mod bridge;
mod client;
mod convert;
mod input_file;
mod media;
mod polling;
pub mod raw;
mod request;

pub(crate) use bridge::Transport;
pub use bridge::Unsupported;
pub use client::{BotApi, BotApiBuilder};
pub use input_file::InputFile;
pub(crate) use media::carried_reply;
pub(crate) use polling::poll;
pub use request::Request;
