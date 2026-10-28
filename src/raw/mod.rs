pub mod core;

#[cfg(feature = "json")]
pub use self::core::json::ToJson;
pub use self::core::{Deserializable, Error, Identifiable, RemoteCall, Serializable};

include!(concat!(env!("OUT_DIR"), "/raw.rs"));
