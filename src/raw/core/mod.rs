pub mod gzip_packed;
pub mod msg_container;
mod primitives;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    UnexpectedEof,
    UnexpectedConstructor { id: u32 },
    InvalidGzip,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UnexpectedEof => f.write_str("unexpected end of TL data"),
            Error::UnexpectedConstructor { id } => write!(f, "unexpected TL constructor {id:#010x}"),
            Error::InvalidGzip => f.write_str("invalid gzip_packed data"),
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
        let mut buf = Vec::new();
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
    type Return: Deserializable;
}
