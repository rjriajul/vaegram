use std::io;

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::crypto::random;
use crate::errors::{Error, Result};

const MAX_PACKET: usize = 16 << 20;

/// MTProto transport framing, see <https://core.telegram.org/mtproto/mtproto-transports>.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Transport {
    /// One-byte length prefix for packets under 508 bytes, four bytes above that.
    Abridged,
    #[default]
    Intermediate,
    /// Intermediate with random padding per packet; required by `dd` MTProxy secrets.
    PaddedIntermediate,
}

impl Transport {
    pub(crate) fn tag(self) -> [u8; 4] {
        match self {
            Transport::Abridged => [0xef; 4],
            Transport::Intermediate => [0xee; 4],
            Transport::PaddedIntermediate => [0xdd; 4],
        }
    }

    pub fn encode(self, payload: &[u8], out: &mut Vec<u8>) {
        let len = payload.len();
        out.reserve(4 + len + 15);
        match self {
            Transport::Abridged => {
                debug_assert!(len % 4 == 0 && len / 4 <= 0xff_ffff);
                let words = len / 4;
                if words < 0x7f {
                    out.push(words as u8);
                } else {
                    out.push(0x7f);
                    out.extend_from_slice(&(words as u32).to_le_bytes()[..3]);
                }
                out.extend_from_slice(payload);
            }
            Transport::Intermediate => {
                out.extend_from_slice(&(len as u32).to_le_bytes());
                out.extend_from_slice(payload);
            }
            Transport::PaddedIntermediate => {
                let noise: [u8; 16] = random();
                let pad = (noise[0] & 15) as usize;
                out.extend_from_slice(&((len + pad) as u32).to_le_bytes());
                out.extend_from_slice(payload);
                out.extend_from_slice(&noise[1..1 + pad]);
            }
        }
    }

    pub async fn read<R: AsyncRead + Unpin>(self, r: &mut R) -> Result<Vec<u8>> {
        let len = match self {
            Transport::Abridged => match r.read_u8().await? {
                0x7f => {
                    let mut words = [0; 4];
                    r.read_exact(&mut words[..3]).await?;
                    u32::from_le_bytes(words) as usize * 4
                }
                words => words as usize * 4,
            },
            Transport::Intermediate | Transport::PaddedIntermediate => r.read_u32_le().await? as usize,
        };
        if len > MAX_PACKET {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("{len}-byte packet is too big")).into());
        }
        let mut payload = vec![0; len];
        r.read_exact(&mut payload).await?;
        if self == Transport::PaddedIntermediate {
            payload.truncate(unpadded_len(&payload));
        }
        if let Ok(code) = <[u8; 4]>::try_from(payload.as_slice()).map(i32::from_le_bytes) {
            if code < 0 {
                return Err(Error::Transport(code.saturating_neg()));
            }
        }
        Ok(payload)
    }
}

// Padding is 0-15 bytes and not marked, so the real length comes from the MTProto packet itself:
// error codes are 4 bytes, plain messages carry their body length, encrypted ones are 24 + 16n.
fn unpadded_len(p: &[u8]) -> usize {
    if p.len() < 24 {
        return p.len().min(4);
    }
    if p[..8] == [0; 8] {
        let body = u32::from_le_bytes([p[16], p[17], p[18], p[19]]) as usize;
        return p.len().min(body.saturating_add(20));
    }
    24 + (p.len() - 24) / 16 * 16
}

#[cfg(test)]
mod tests {
    use super::Transport;
    use crate::errors::Error;

    #[tokio::test]
    async fn the_most_negative_error_code_does_not_overflow() {
        let frame = [4u32.to_le_bytes(), i32::MIN.to_le_bytes()].concat();
        let read = Transport::Intermediate.read(&mut &frame[..]).await;
        assert!(matches!(read, Err(Error::Transport(i32::MAX))), "{read:?}");
    }
}
