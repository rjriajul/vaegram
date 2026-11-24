//! A bot that talks to Telegram through the Bot API over HTTPS (feature `bot-api`), instead of an MTProto session.
//!
//! [`BotApi::invoke`] sends any method of [`raw::methods`]; the answers are the types of [`raw::types`], generated
//! from the Bot API's specification by `tools/botapi/gen.py`.
//!
//! Experimental: part of the 0.4.0 nightlies and may change in any of them.

mod client;
mod input_file;
pub mod raw;
mod request;

pub use client::{BotApi, BotApiBuilder};
pub use input_file::InputFile;
pub use request::Request;
