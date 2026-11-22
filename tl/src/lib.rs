//! The Telegram API as TL types, generated at build time from the schema in `compiler/source`, with the runtime
//! that writes and reads them. [vaegram](https://docs.rs/vaegram) re-exports this crate as `vaegram::raw`; it has
//! its own crate so the generated code is compiled once and cached, not with every change to the client.

// The generated `raw.rs` below documents nothing: the TL schema carries no descriptions. An attribute on the
// `include!` itself is ignored, so the allow covers this crate, and `core` turns the lint back on.
#![allow(missing_docs)]

pub mod core;

#[cfg(feature = "json")]
pub use self::core::json::ToJson;
pub use self::core::{Deserializable, Error, Identifiable, RemoteCall, Serializable};

include!(concat!(env!("OUT_DIR"), "/raw.rs"));
