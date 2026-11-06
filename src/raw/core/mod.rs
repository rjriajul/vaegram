//! The TL runtime under the generated types: the traits that write and read them, the error a read fails with,
//! and the two MTProto wrappers a session unpacks itself.
#![warn(missing_docs)]

/// `gzip_packed`, the wrapper Telegram compresses big answers in.
pub mod gzip_packed;
#[cfg(feature = "json")]
pub mod json;
/// `msg_container`, which carries several messages in one.
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

/// Why TL bytes couldn't be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Error {
    /// The bytes ended in the middle of a value.
    UnexpectedEof,
    /// A constructor id that isn't one of the expected type's, such as one from a newer layer.
    UnexpectedConstructor {
        /// The constructor id that was found.
        id: u32,
    },
    /// A `gzip_packed` whose data doesn't decompress.
    InvalidGzip,
    /// Objects nested more than 64 deep, refused so hostile data can't overflow the stack.
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

/// A TL constructor with a fixed id.
pub trait Identifiable {
    /// The constructor's 32-bit id, as written in the schema after `#`.
    const CONSTRUCTOR_ID: u32;
}

/// A value that can be written as TL.
pub trait Serializable {
    /// Appends the value's TL bytes to `buf`, little-endian and with the constructor id in front for boxed types
    /// and constructors.
    fn serialize(&self, buf: &mut Vec<u8>);

    /// The value's TL bytes in a new buffer.
    fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(128);
        self.serialize(&mut buf);
        buf
    }
}

/// A value that can be read from TL.
pub trait Deserializable: Sized {
    /// Reads one value from the front of `buf` and advances `buf` past it.
    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error>;

    /// Reads one value from the start of `bytes`; bytes after it are ignored.
    fn from_bytes(mut bytes: &[u8]) -> Result<Self, Error> {
        Self::deserialize(&mut bytes)
    }
}

/// A TL function: a request that can be sent to Telegram with `Client::invoke`.
pub trait RemoteCall: Serializable {
    /// The TL name, as in `messages.sendMessage`.
    const NAME: &'static str;
    /// The type Telegram answers the request with.
    type Return: Deserializable;
}
