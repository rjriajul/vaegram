pub mod gzip_packed;
#[cfg(feature = "json")]
pub mod json;
pub mod msg_container;
mod primitives;

use std::cell::Cell;

const MAX_DEPTH: u32 = 64;

thread_local! {
    static DEPTH: Cell<u32> = const { Cell::new(0) };
}

pub(crate) fn nested<T>(f: impl FnOnce() -> Result<T, Error>) -> Result<T, Error> {
    let depth = DEPTH.get();
    if depth >= MAX_DEPTH {
        return Err(Error::TooDeep);
    }
    DEPTH.set(depth + 1);
    let r = f();
    DEPTH.set(depth);
    r
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    UnexpectedEof,
    UnexpectedConstructor { id: u32 },
    InvalidGzip,
    TooDeep,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UnexpectedEof => f.write_str("unexpected end of TL data"),
            Error::UnexpectedConstructor { id } => write!(f, "unexpected TL constructor {id:#010x}"),
            Error::InvalidGzip => f.write_str("invalid gzip_packed data"),
            Error::TooDeep => f.write_str("TL data nests too deeply"),
        }
    }
}

impl std::error::Error for Error {}

pub trait Identifiable {
    const CONSTRUCTOR_ID: u32;
}

pub trait Serializable {
    fn serialize(&self, buf: &mut Vec<u8>);

    fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(128);
        self.serialize(&mut buf);
        buf
    }
}

pub trait Deserializable: Sized {
    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error>;

    fn from_bytes(mut bytes: &[u8]) -> Result<Self, Error> {
        Self::deserialize(&mut bytes)
    }
}

pub trait RemoteCall: Serializable {
    /// The TL name, as in `messages.sendMessage`.
    const NAME: &'static str;
    type Return: Deserializable;
}
