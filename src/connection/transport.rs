use std::io;

use crate::crypto::random;
use crate::errors::{Error, Result};

const MAX_PACKET: usize = 16 << 20;

/// Read bytes; taken packets leave the front only once they fill half the buffer, so each byte moves once.
#[derive(Debug, Default)]
pub struct RecvBuf {
    data: Vec<u8>,
    start: usize,
    #[cfg(test)]
    moved: usize,
}

impl RecvBuf {
    pub fn extend_from_slice(&mut self, bytes: &[u8]) {
        self.make_room().extend_from_slice(bytes);
    }

    pub fn unread(&self) -> &[u8] {
        &self.data[self.start..]
    }

    pub(crate) fn make_room(&mut self) -> &mut Vec<u8> {
        if self.start == self.data.len() {
            self.data.clear();
            self.start = 0;
        } else if self.start >= self.data.len() / 2 {
            #[cfg(test)]
            {
                self.moved += self.data.len() - self.start;
            }
            self.data.drain(..self.start);
            self.start = 0;
        }
        &mut self.data
    }
}

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
    /// Free bytes [`Transport::frame`] needs in front of the payload.
    pub const ROOM: usize = 4;

    pub(crate) fn tag(self) -> [u8; 4] {
        match self {
            Transport::Abridged => [0xef; 4],
            Transport::Intermediate => [0xee; 4],
            Transport::PaddedIntermediate => [0xdd; 4],
        }
    }

    /// Frames the payload in `buf[Self::ROOM..]` in place and returns where the frame starts.
    pub fn frame(self, buf: &mut Vec<u8>) -> usize {
        let len = buf.len() - Self::ROOM;
        match self {
            Transport::Abridged => {
                debug_assert!(len % 4 == 0 && len / 4 <= 0xff_ffff);
                let words = len / 4;
                if words < 0x7f {
                    buf[3] = words as u8;
                    3
                } else {
                    buf[0] = 0x7f;
                    buf[1..4].copy_from_slice(&(words as u32).to_le_bytes()[..3]);
                    0
                }
            }
            Transport::Intermediate => {
                buf[..4].copy_from_slice(&(len as u32).to_le_bytes());
                0
            }
            Transport::PaddedIntermediate => {
                let noise: [u8; 16] = random();
                let pad = (noise[0] & 15) as usize;
                buf.extend_from_slice(&noise[1..1 + pad]);
                buf[..4].copy_from_slice(&((len + pad) as u32).to_le_bytes());
                0
            }
        }
    }

    /// Takes one whole packet off the front of `buf`, or `None` while it is still arriving.
    pub fn decode(self, recv: &mut RecvBuf) -> Result<Option<Vec<u8>>> {
        let buf = recv.unread();
        let (head, len) = match (self, buf.first()) {
            (_, None) => return Ok(None),
            (Transport::Abridged, Some(&words)) if words != 0x7f => (1, words as usize * 4),
            _ if buf.len() < 4 => return Ok(None),
            (Transport::Abridged, _) => (4, u32::from_le_bytes([buf[1], buf[2], buf[3], 0]) as usize * 4),
            _ => (4, u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize),
        };
        if len > MAX_PACKET {
            return Err(io::Error::new(io::ErrorKind::InvalidData, format!("{len}-byte packet is too big")).into());
        }
        if buf.len() < head + len {
            return Ok(None);
        }
        let mut payload = buf[head..head + len].to_vec();
        recv.start += head + len;
        if self == Transport::PaddedIntermediate {
            payload.truncate(unpadded_len(&payload));
        }
        if let Ok(code) = <[u8; 4]>::try_from(&payload[..]).map(i32::from_le_bytes) {
            if code < 0 {
                return Err(Error::Transport(code.saturating_neg()));
            }
        }
        Ok(Some(payload))
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
    use super::*;

    fn framed(transport: Transport, payload: &[u8]) -> Vec<u8> {
        let mut buf = vec![0; Transport::ROOM];
        buf.extend_from_slice(payload);
        let start = transport.frame(&mut buf);
        buf.drain(..start);
        buf
    }

    fn trickle(transport: Transport, frame: &[u8]) -> Result<Vec<u8>> {
        let mut buf = RecvBuf::default();
        for (i, &b) in frame.iter().enumerate() {
            buf.extend_from_slice(&[b]);
            if let Some(packet) = transport.decode(&mut buf)? {
                assert_eq!(i, frame.len() - 1, "{transport:?} returned a packet early");
                assert!(buf.unread().is_empty());
                return Ok(packet);
            }
        }
        panic!("{transport:?} never returned the packet");
    }

    #[test]
    fn packets_round_trip_across_length_boundaries() {
        for transport in [Transport::Abridged, Transport::Intermediate] {
            for words in [0, 1, 0x7e, 0x7f, 0x80, 0x3000] {
                let payload: Vec<u8> = (0..words * 4).map(|i| i as u8).collect();
                let packet = trickle(transport, &framed(transport, &payload)).unwrap();
                assert_eq!(packet[..], payload[..], "{transport:?} {words}");
            }
        }
    }

    #[test]
    fn several_packets_in_one_read() {
        let mut buf = RecvBuf::default();
        for i in 0..3u8 {
            buf.extend_from_slice(&framed(Transport::Intermediate, &[i; 8]));
        }
        buf.extend_from_slice(&[8, 0]);
        for i in 0..3u8 {
            assert_eq!(Transport::Intermediate.decode(&mut buf).unwrap().unwrap()[..], [i; 8]);
        }
        assert!(Transport::Intermediate.decode(&mut buf).unwrap().is_none());
        assert_eq!(buf.unread(), [8, 0]);
    }

    #[test]
    fn padded_reads_strip_any_padding() {
        let encrypted: Vec<u8> = [1u8; 8].into_iter().chain((0..16 + 32).map(|i| i as u8)).collect();
        let mut plain = vec![0u8; 16];
        plain.extend_from_slice(&8u32.to_le_bytes());
        plain.extend_from_slice(b"abcd1234");
        for payload in [encrypted, plain] {
            for _ in 0..16 {
                let packet = trickle(Transport::PaddedIntermediate, &framed(Transport::PaddedIntermediate, &payload));
                assert_eq!(packet.unwrap()[..], payload[..]);
            }
            for pad in [0, 3, 4, 15] {
                let mut frame = ((payload.len() + pad) as u32).to_le_bytes().to_vec();
                frame.extend_from_slice(&payload);
                frame.extend_from_slice(&[0xaa; 15][..pad]);
                assert_eq!(trickle(Transport::PaddedIntermediate, &frame).unwrap()[..], payload[..], "pad {pad}");
            }
        }
    }

    #[test]
    fn frame_headers() {
        let frame = framed(Transport::Abridged, &[0; 0x7e * 4]);
        assert_eq!((frame[0], frame.len()), (0x7e, 1 + 0x7e * 4));
        assert_eq!(framed(Transport::Abridged, &[0; 0x7f * 4])[..4], [0x7f, 0x7f, 0, 0]);
        assert_eq!(framed(Transport::Abridged, &vec![0; 0x12345 * 4])[..4], [0x7f, 0x45, 0x23, 0x01]);
        assert_eq!(framed(Transport::Intermediate, b"abcd1234")[..], *b"\x08\0\0\0abcd1234");

        for _ in 0..64 {
            let mut frame = vec![0; Transport::ROOM];
            frame.extend_from_slice(b"abcd1234");
            assert_eq!(Transport::PaddedIntermediate.frame(&mut frame), 0);
            let len = u32::from_le_bytes(frame[..4].try_into().unwrap()) as usize;
            assert_eq!(frame.len(), 4 + len);
            assert!((8..24).contains(&len));
            assert_eq!(&frame[4..12], b"abcd1234");
        }
    }

    #[test]
    fn error_codes_surface() {
        for transport in [Transport::Abridged, Transport::Intermediate, Transport::PaddedIntermediate] {
            let frame = framed(transport, &(-404i32).to_le_bytes());
            assert!(matches!(trickle(transport, &frame), Err(Error::Transport(404))), "{transport:?}");
        }
    }

    #[test]
    fn rejects_oversized_packets() {
        let mut buf = RecvBuf::default();
        buf.extend_from_slice(&[0x7f, 0xff, 0xff, 0xff]);
        assert!(matches!(Transport::Abridged.decode(&mut buf), Err(Error::Io(_))));
    }

    #[test]
    fn a_burst_of_packets_moves_each_byte_at_most_once() {
        let frame = framed(Transport::Intermediate, &[7; 1000]);
        let mut buf = RecvBuf::default();
        let mut total = 0;
        for _ in 0..200 {
            for chunk in frame.repeat(9).chunks(4096) {
                buf.extend_from_slice(chunk);
                total += chunk.len();
                while let Some(packet) = Transport::Intermediate.decode(&mut buf).unwrap() {
                    assert_eq!(packet.len(), 1000);
                }
            }
        }
        assert!(buf.unread().is_empty());
        assert!(buf.moved <= total, "{} bytes moved for {total} read", buf.moved);
    }

    #[test]
    fn the_most_negative_error_code_does_not_overflow() {
        let frame = [4u32.to_le_bytes(), i32::MIN.to_le_bytes()].concat();
        let read = trickle(Transport::Intermediate, &frame);
        assert!(matches!(read, Err(Error::Transport(i32::MAX))), "{read:?}");
    }
}
