use super::{Deserializable, Error};

/// The constructor id of `msg_container`.
pub const ID: u32 = 0x73f1f8dc;

/// One message in a container, its body borrowed from the container.
pub struct Message<'a> {
    /// The message's id.
    pub msg_id: i64,
    /// The message's sequence number.
    pub seq_no: i32,
    /// The message's TL bytes.
    pub body: &'a [u8],
}

/// The messages of a whole `msg_container` object, in order, read from after its constructor id (which isn't
/// checked). Fails with [`Error::UnexpectedEof`] when a message runs past the end.
pub fn parse(body: &[u8]) -> Result<Vec<Message<'_>>, Error> {
    let mut buf = body.get(4..).ok_or(Error::UnexpectedEof)?;
    let count = u32::deserialize(&mut buf)?;
    let mut messages = Vec::new();
    for _ in 0..count {
        let msg_id = i64::deserialize(&mut buf)?;
        let seq_no = i32::deserialize(&mut buf)?;
        let len = u32::deserialize(&mut buf)? as usize;
        if buf.len() < len {
            return Err(Error::UnexpectedEof);
        }
        let (body, rest) = buf.split_at(len);
        buf = rest;
        messages.push(Message { msg_id, seq_no, body });
    }
    Ok(messages)
}
