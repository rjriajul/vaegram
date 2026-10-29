use super::{Deserializable, Error, Serializable};

const VECTOR_ID: u32 = 0x1cb5c415;
const BOOL_TRUE_ID: u32 = 0x997275b5;
const BOOL_FALSE_ID: u32 = 0xbc799737;

fn take<'a>(buf: &mut &'a [u8], n: usize) -> Result<&'a [u8], Error> {
    if buf.len() < n {
        return Err(Error::UnexpectedEof);
    }
    let (head, rest) = buf.split_at(n);
    *buf = rest;
    Ok(head)
}

macro_rules! le_number {
    ($($t:ty),*) => {$(
        impl Serializable for $t {
            fn serialize(&self, buf: &mut Vec<u8>) {
                buf.extend_from_slice(&self.to_le_bytes());
            }
        }
        impl Deserializable for $t {
            fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
                Ok(<$t>::from_le_bytes(take(buf, size_of::<$t>())?.try_into().unwrap()))
            }
        }
    )*};
}

le_number!(u32, i32, i64, f64);

impl<const N: usize> Serializable for [u8; N] {
    fn serialize(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(self);
    }
}

impl<const N: usize> Deserializable for [u8; N] {
    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
        Ok(take(buf, N)?.try_into().unwrap())
    }
}

impl Serializable for bool {
    fn serialize(&self, buf: &mut Vec<u8>) {
        if *self { BOOL_TRUE_ID } else { BOOL_FALSE_ID }.serialize(buf);
    }
}

impl Deserializable for bool {
    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
        match u32::deserialize(buf)? {
            BOOL_TRUE_ID => Ok(true),
            BOOL_FALSE_ID => Ok(false),
            id => Err(Error::UnexpectedConstructor { id }),
        }
    }
}

impl Serializable for [u8] {
    fn serialize(&self, buf: &mut Vec<u8>) {
        assert!(self.len() < 1 << 24, "TL bytes can't be 16 MiB or longer; split the data first");
        let header = if self.len() < 254 { 1 } else { 4 };
        let end = buf.len() + (header + self.len()).next_multiple_of(4);
        // Reserve the padding with the data, so a full buffer doesn't double just to fit up to 3 zeros.
        buf.reserve(end - buf.len());
        if header == 1 {
            buf.push(self.len() as u8);
        } else {
            buf.push(254);
            buf.extend_from_slice(&(self.len() as u32).to_le_bytes()[..3]);
        }
        buf.extend_from_slice(self);
        buf.resize(end, 0);
    }
}

impl Serializable for Vec<u8> {
    fn serialize(&self, buf: &mut Vec<u8>) {
        self.as_slice().serialize(buf);
    }
}

fn read_bytes<'a>(buf: &mut &'a [u8]) -> Result<&'a [u8], Error> {
    let (len, header) = match take(buf, 1)?[0] {
        254 => {
            let b = take(buf, 3)?;
            (u32::from_le_bytes([b[0], b[1], b[2], 0]) as usize, 4)
        }
        n => (n as usize, 1),
    };
    let data = take(buf, len)?;
    take(buf, (4 - (header + len) % 4) % 4)?;
    Ok(data)
}

impl Deserializable for Vec<u8> {
    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
        Ok(read_bytes(buf)?.to_vec())
    }
}

impl Serializable for String {
    fn serialize(&self, buf: &mut Vec<u8>) {
        self.as_bytes().serialize(buf);
    }
}

impl Deserializable for String {
    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
        Ok(String::from_utf8_lossy(read_bytes(buf)?).into_owned())
    }
}

impl<T: Serializable> Serializable for Vec<T> {
    fn serialize(&self, buf: &mut Vec<u8>) {
        VECTOR_ID.serialize(buf);
        (self.len() as i32).serialize(buf);
        for item in self {
            item.serialize(buf);
        }
    }
}

impl<T: Deserializable> Deserializable for Vec<T> {
    fn deserialize(buf: &mut &[u8]) -> Result<Self, Error> {
        match u32::deserialize(buf)? {
            VECTOR_ID => {}
            id => return Err(Error::UnexpectedConstructor { id }),
        }
        let len = u32::deserialize(buf)?;
        let mut items = Vec::with_capacity(capacity::<T>(len, buf.len()));
        for _ in 0..len {
            items.push(T::deserialize(buf)?);
        }
        Ok(items)
    }
}

// `len` comes from the network: cap it by what the input can hold (4+ bytes per value), and the allocation by the
// input's size, since a value can take more room in memory than on the wire.
fn capacity<T>(len: u32, input: usize) -> usize {
    (len as usize).min(input / size_of::<T>().max(4))
}

#[cfg(test)]
mod tests {
    use super::capacity;

    #[test]
    fn a_claimed_vector_length_allocates_no_more_than_the_input() {
        let input = 4096;
        assert!(capacity::<String>(u32::MAX, input) * size_of::<String>() <= input);
        assert!(capacity::<Vec<u8>>(u32::MAX, input) * size_of::<Vec<u8>>() <= input);
        assert_eq!((capacity::<i32>(u32::MAX, input), capacity::<i32>(3, input)), (1024, 3));
    }
}
