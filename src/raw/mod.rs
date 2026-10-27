pub mod core;

pub use self::core::{Deserializable, Error, Identifiable, RemoteCall, Serializable};

include!(concat!(env!("OUT_DIR"), "/raw.rs"));
