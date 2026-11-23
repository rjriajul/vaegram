// The generated `raw.rs` below documents nothing: the TL schema carries no descriptions. An attribute on the
// `include!` itself is ignored, so the allow covers this module, and `core` turns the lint back on.
#![allow(missing_docs)]

pub mod core;

#[cfg(feature = "json")]
pub use self::core::json::ToJson;
pub use self::core::{Deserializable, Error, Identifiable, RemoteCall, Serializable};

include!(concat!(env!("OUT_DIR"), "/raw.rs"));
