use std::io::Read;

use super::{Deserializable, Error};

pub const ID: u32 = 0x3072cfa1;

/// Cap on the unpacked size, so a tiny packet can't make us allocate gigabytes.
const MAX_UNPACKED: u64 = 32 << 20;

pub fn unpack(body: &[u8]) -> Result<Vec<u8>, Error> {
    let mut buf = body.get(4..).ok_or(Error::UnexpectedEof)?;
    let packed = Vec::<u8>::deserialize(&mut buf)?;
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(packed.as_slice())
        .take(MAX_UNPACKED)
        .read_to_end(&mut out)
        .map_err(|_| Error::InvalidGzip)?;
    Ok(out)
}
