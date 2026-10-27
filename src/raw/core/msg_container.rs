use super::{Deserializable, Error};

pub const ID: u32 = 0x73f1f8dc;

pub struct Message<'a> {
    pub msg_id: i64,
    pub seq_no: i32,
    pub body: &'a [u8],
}

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
